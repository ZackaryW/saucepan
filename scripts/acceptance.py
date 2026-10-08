"""Build two real releases and exercise the isolated, inspectable mock installation.

Run with python scripts/acceptance.py. Never deletes an existing mock root.
Fixtures and build copies live under target/acceptance, not inside package data.
"""
from __future__ import annotations
import hashlib
import http.server
import json
import os
from pathlib import Path
import shutil
import subprocess
import threading
import time
import uuid
import zipfile

REPO = Path(__file__).resolve().parents[1]
ROOT = REPO / "mock"
RUN = REPO / "target" / "acceptance" / str(uuid.uuid4())
EXT = ".exe" if os.name == "nt" else ""
NAME = "saucepan" + EXT
RUN.mkdir(parents=True)


def digest(path):
    with open(path, "rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def build(label):
    env = dict(os.environ, SAUCEPAN_BUILD_ID=label)
    subprocess.run(["cargo", "build", "--release", "--bins"], cwd=REPO, env=env, check=True)
    directory = RUN / label
    directory.mkdir()
    exe = directory / NAME
    shutil.copy2(REPO / "target" / "release" / NAME, exe)
    archive = directory / "saucepan.zip"
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
        bundle.write(exe, NAME)
    return exe, archive


print("Building distinct application releases", flush=True)
exe_a, zip_a = build("acceptance-a")
exe_b, zip_b = build("acceptance-b")
assert digest(exe_a) != digest(exe_b)
bad_zip = RUN / "bad.zip"
with zipfile.ZipFile(bad_zip, "w") as bundle:
    bundle.writestr(NAME, b"not an executable")
archives = {1: zip_a, 2: zip_b, 3: bad_zip}
state = {"version": 1, "checks": 0, "fail": False}


class Fixture(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path.startswith("/repos/"):
            state["checks"] += 1
            if state["fail"]:
                self.send_error(503)
                return
            n = state["version"]
            data = json.dumps({"id": n, "tag_name": f"v{n}", "draft": False,
                               "prerelease": False, "assets": [{"id": n,
                               "name": "saucepan.zip", "browser_download_url": f"{base}/asset/{n}",
                               "digest": "sha256:" + digest(archives[n])}]}).encode()
        elif self.path.startswith("/asset/"):
            data = archives[int(self.path.rsplit("/", 1)[1])].read_bytes()
        else:
            self.send_error(404)
            return
        self.send_response(200)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Content-Type", "application/octet-stream")
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *_):
        pass


server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
base = f"http://127.0.0.1:{server.server_port}"
threading.Thread(target=server.serve_forever, daemon=True).start()
now = 100


def call(*args, executable=exe_b, ok=True, raw=False, force=False):
    command = [str(executable), "--root", str(ROOT), "--test-github-api", base, "--test-now", str(now)]
    if force:
        command.append("--force-refresh")
    result = subprocess.run(command + list(map(str, args)), cwd=REPO, capture_output=True, text=True, timeout=90)
    if ok:
        assert result.returncode == 0, result.stderr
    else:
        assert result.returncode != 0, result.stdout
        return result.stderr.strip()
    return result.stdout.strip() if raw else json.loads(result.stdout)


def payload(name, value):
    path = RUN / name
    path.write_text(json.dumps(value, indent=2), encoding="utf-8")
    return path


def record(entry):
    return call("inspect", entry)


def await_file(path):
    end = time.monotonic() + 30
    while not path.exists():
        assert time.monotonic() < end, f"timeout waiting for {path}"
        time.sleep(0.02)


try:
    if ROOT.exists() and any(ROOT.iterdir()):
        # Open and inspect every existing record before adding this run's fixtures.
        assert all((ROOT / p).is_dir() for p in ("source", "index", "store", "bin", ".locks", ".staging"))
        for index in (ROOT / "index").iterdir():
            record(index.name)
        print("Reusing compatible mock; preserving all existing contents", flush=True)
    else:
        call("init")
    # Unique fixture source per run keeps prior source metadata inspectable.
    source = call("register-source", payload("source.json", {"type": "github_release", "repo": f"fixture/saucepan-{RUN.name}"}))
    platform = {"os": "windows" if os.name == "nt" else ("macos" if __import__("sys").platform == "darwin" else "linux"),
                "arch": "aarch64" if __import__("platform").machine().lower() in ("arm64", "aarch64") else "x86_64"}
    request = {"source_uuid": source["uuid"], "selector": {"type": "latest_stable"},
               "selection": {"platform": platform, "format": "zip", "bin": [NAME],
                             "filters": [{"platform": platform, "asset_patterns": ["saucepan.zip"]}]},
               "policies": {"eviction": {"strategy": "latest", "keep": 1}, "protection": {"type": "useable_app"}}}
    request_path = payload("request.json", request)
    probe = payload("probe.json", {"args": ["--version"], "stdout_contains": "saucepan ", "timeout_seconds": 10})
    shim = ROOT / "bin" / NAME
    built_shim = REPO / "target" / "release" / ("saucepan-shim" + EXT)
    if shim.exists():
        config = json.loads(shim.with_suffix(".shim.json").read_text())
        app = config["app"]
        claimed = call("acquire", request_path, "--app", app)
        call("activate", app, claimed["claim"]["entry_uuid"], NAME, probe)
        boot = {"app": app, "claimed": claimed}
    else:
        operation = call("bootstrap", request_path, built_shim)
        boot = {"app": operation["app"], "claimed": {"claim": {"uuid": operation["claim"], "entry_uuid": operation["candidate"]}}}
    app, claim = boot["app"], boot["claimed"]["claim"]["uuid"]
    a = boot["claimed"]["claim"]["entry_uuid"]
    assert digest(ROOT / "store" / a / NAME) == digest(exe_a)
    version_a = call("--version", executable=shim, raw=True)
    assert "acceptance-a" in version_a
    initial_checks = state["checks"]
    state["version"] = 2
    now = 86499
    cached = call("update-claim", app, claim)
    assert cached["claim"]["entry_uuid"] == a and state["checks"] == initial_checks
    ready, release = RUN / "ready", RUN / "release"
    old = subprocess.Popen([str(shim), "--root", str(ROOT), "test-hold", str(ready), str(release)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    await_file(ready)
    assert ready.read_text() == "acceptance-a"
    now = 86500
    update = call("self-update", app, claim)
    b = update["candidate"]
    assert a != b and state["checks"] == initial_checks + 1
    assert digest(ROOT / "store" / b / NAME) == digest(exe_b)
    version_b = call("--version", executable=shim, raw=True)
    assert "acceptance-b" in version_b
    assert a in call("reclaim")["deferred"]
    call("activate", app, a, NAME, probe)
    assert "acceptance-a" in call("--version", executable=shim, raw=True)
    call("activate", app, b, NAME, probe)
    state["version"] = 3
    failure = call("self-update", app, claim, force=True, ok=False)
    assert record(app)["active"]["entry_uuid"] == b
    assert "acceptance-b" in call("--version", executable=shim, raw=True)
    release.write_text("exit")
    out, err = old.communicate(timeout=30)
    assert old.returncode == 0, err
    old_result = json.loads(out)
    assert old_result["build"] == "acceptance-a" and os.path.samefile(old_result["root"], ROOT), old_result
    call("reclaim")
    assert record(a)["state"] == "reclaimed"
    state["version"] = 2
    call("self-update", app, claim, force=True)
    # Two independent ordinary app claims and eleven-entry retention.
    ordinary = json.loads(json.dumps(request))
    ordinary["policies"] = {"identical": "new_entry"}
    ordinary["selection"]["bin"] = []  # generic archive content is a separate item
    ordinary_path = payload("ordinary.json", ordinary)
    app_a = call("register-app", f"consumer-a-{RUN.name}")["uuid"]
    app_b = call("register-app", f"consumer-b-{RUN.name}")["uuid"]
    first = call("acquire", ordinary_path, "--app", app_a)
    oldest = first["claim"]["entry_uuid"]
    other = call("acquire", ordinary_path, "--app", app_b)
    for _ in range(9):
        call("acquire", ordinary_path)
    assert record(oldest)["state"] == "reclaimed"
    assert record(app_a)["claims"][first["claim"]["uuid"]]["entry_uuid"] == oldest
    refreshed = call("update-claim", app_a, first["claim"]["uuid"])
    assert refreshed["claim"]["entry_uuid"] != oldest
    assert record(app_b)["claims"][other["claim"]["uuid"]]["entry_uuid"] == other["claim"]["entry_uuid"]
    reuse = json.loads(json.dumps(ordinary)); reuse["policies"]["identical"] = "reuse"
    reused = call("acquire", payload("reuse.json", reuse))
    assert reused["reused"]
    state["fail"] = True
    call("update-claim", app, claim, force=True, ok=False)
    stale = call("--allow-stale", "update-claim", app, claim, force=True)
    assert stale["acquired"]["stale"]
    state["fail"] = False
    assert not (ROOT / "source" / source["uuid"]).exists()
    call("--version", executable=shim, raw=True)
    report = {"root": str(ROOT), "run": str(RUN), "platform": platform, "build_a": {"version": version_a, "sha256": digest(exe_a), "entry": a},
              "build_b": {"version": version_b, "sha256": digest(exe_b), "entry": b}, "app": app, "claim": claim,
              "active": record(app)["active"], "old_process": old_result, "failed_candidate": failure,
              "oldest_reclaimed": oldest, "reacquired": refreshed["claim"]["entry_uuid"], "discovery_checks": state["checks"],
              "root_isolation": "all commands explicit root; child root matched; no production-root fallback", "result": "passed"}
    (RUN / "report.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    (REPO / "target" / "acceptance" / "latest.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, indent=2), flush=True)
finally:
    if "old" in globals() and old.poll() is None:
        release.write_text("exit", encoding="utf-8")
        old.communicate(timeout=30)
    server.shutdown()
