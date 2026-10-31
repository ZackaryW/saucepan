"""Build two real releases and exercise an isolated, inspectable installation.

Run with python scripts/acceptance.py. Each run initializes its own root under
target/acceptance/RUN; no existing root is opened or deleted. Freshness is driven
by the explicit --test-now clock and registered policies.
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
RUN = REPO / "target" / "acceptance" / str(uuid.uuid4())
ROOT = RUN / "root"
DAY = 86400
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
        if "/releases/assets/" in self.path:
            data = archives[int(self.path.rsplit("/", 1)[1])].read_bytes()
        elif self.path.startswith("/repos/"):
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


def call(*args, executable=exe_b, ok=True, raw=False):
    command = [str(executable), "--root", str(ROOT), "--test-github-api", base, "--test-now", str(now)]
    result = subprocess.run(command + list(map(str, args)), cwd=REPO, capture_output=True, text=True, timeout=90)
    if ok:
        assert result.returncode == 0, result.stderr
    else:
        assert result.returncode != 0, result.stdout
        # Lifecycle outcomes are recorded JSON on stdout (exit 2); errors use stderr.
        return json.loads(result.stdout) if result.stdout.strip() else result.stderr.strip()
    return result.stdout.strip() if raw else json.loads(result.stdout)


def payload(name, value):
    path = RUN / name
    path.write_text(json.dumps(value, indent=2), encoding="utf-8")
    return path


def record(entry):
    return call("inspect", entry)


def claims(app):
    return {view["claim"]["uuid"]: view["claim"] for view in call("claims", app)}


def await_file(path):
    end = time.monotonic() + 30
    while not path.exists():
        assert time.monotonic() < end, f"timeout waiting for {path}"
        time.sleep(0.02)


try:
    call("init")
    # Unique fixture source per run keeps prior source metadata inspectable.
    source = call("register-source", payload("source.json", {"type": "github_release", "repo": f"fixture/saucepan-{RUN.name}"}))
    platform = {"os": "windows" if os.name == "nt" else ("macos" if __import__("sys").platform == "darwin" else "linux"),
                "arch": "aarch64" if __import__("platform").machine().lower() in ("arm64", "aarch64") else "x86_64"}
    # Profiles expand to flat policies; each run customizes one leaf per request family.
    keep_one = {"uuid": str(uuid.uuid4()), "type": "revision_policy", "history": "latest", "keep": 1}
    app_profile = call("profile", "managed", "--for-install", "--catalog", payload("app-custom.json", [keep_one]),
                       "--acquisition-policy", keep_one["uuid"])
    new_entry = {"uuid": str(uuid.uuid4()), "type": "identical_pull_policy", "strategy": "new_entry"}
    ordinary_profile = call("profile", "managed", "--catalog", payload("ordinary-custom.json", [new_entry]),
                            "--acquisition-policy", new_entry["uuid"])
    reuse_profile = call("profile", "managed")
    allow_stale = {"uuid": str(uuid.uuid4()), "type": "stale_policy", "allow_stale": True}
    stale_profile = call("profile", "managed", "--catalog", payload("stale-custom.json", [allow_stale]),
                         "--retrieval-policy", allow_stale["uuid"])
    asset_filter = {"uuid": str(uuid.uuid4()), "type": "github_release_asset_filter", "asset_patterns": ["saucepan.zip"]}
    catalog = {policy["uuid"]: policy for profile in (app_profile, ordinary_profile, reuse_profile, stale_profile)
               for policy in profile["catalog"]}
    call("register-policies", payload("policies.json", [*catalog.values(), asset_filter]))
    request = {"source_uuid": source["uuid"], "selector": {"type": "latest_stable"},
               "selection": {"format": "zip", "bin": [NAME], "filter_refs": [asset_filter["uuid"]]},
               "policy_refs": app_profile["combined_policy_refs"]}
    request_path = payload("request.json", request)
    probe = payload("probe.json", {"args": ["--version"], "stdout_contains": "saucepan ", "timeout_seconds": 10})
    shim = ROOT / "bin" / NAME
    built_shim = REPO / "target" / "release" / ("saucepan-shim" + EXT)
    boot = call("bootstrap", request_path, built_shim, "--operation", uuid.uuid4())
    assert boot["status"] == "complete", boot
    app, claim, a = boot["app"], boot["claim"], boot["candidate"]
    assert digest(ROOT / "store" / a / NAME) == digest(exe_a)
    version_a = call("--version", executable=shim, raw=True)
    assert "acceptance-a" in version_a
    initial_checks = state["checks"]
    state["version"] = 2
    now = 100 + DAY - 1
    cached = call("update-claim", app, claim)
    assert cached["claim"]["entry_uuid"] == a and state["checks"] == initial_checks
    ready, release = RUN / "ready", RUN / "release"
    old = subprocess.Popen([str(shim), "--root", str(ROOT), "test-hold", str(ready), str(release)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    await_file(ready)
    assert ready.read_text() == "acceptance-a"
    now = 100 + DAY
    update = call("self-update", app, claim)
    b = update["candidate"]
    assert a != b and state["checks"] == initial_checks + 1
    assert digest(ROOT / "store" / b / NAME) == digest(exe_b)
    version_b = call("--version", executable=shim, raw=True)
    assert "acceptance-b" in version_b
    assert a in call("reclaim")["deferred"]
    # Activation follows the claim; rollback moves the claim and binding together.
    assert call("rollback", app, claim, a, probe)["status"] == "complete"
    assert "acceptance-a" in call("--version", executable=shim, raw=True)
    assert call("rollback", app, claim, b, probe)["status"] == "complete"
    # An expired discovery observes release 3, whose executable fails the probe.
    state["version"] = 3
    now += DAY
    failure = call("self-update", app, claim, ok=False)
    assert failure["status"] == "failed", failure
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
    now += DAY
    call("self-update", app, claim)
    # Two independent ordinary app claims and eleven-entry retention.
    ordinary = json.loads(json.dumps(request))
    ordinary["policy_refs"] = ordinary_profile["combined_policy_refs"]
    ordinary["selection"]["bin"] = []  # generic archive content is a separate item
    ordinary_path = payload("ordinary.json", ordinary)
    app_a = call("register-app", f"consumer-a-{RUN.name}")["uuid"]
    app_b = call("register-app", f"consumer-b-{RUN.name}")["uuid"]
    first = call("acquire", ordinary_path, "--app", app_a, "--scope", "acceptance/ordinary")
    oldest = first["claim"]["entry_uuid"]
    other = call("acquire", ordinary_path, "--app", app_b, "--scope", "acceptance/ordinary")
    for _ in range(9):
        call("acquire", ordinary_path)
    assert record(oldest)["state"] == "reclaimed"
    assert claims(app_a)[first["claim"]["uuid"]]["entry_uuid"] == oldest
    refreshed = call("update-claim", app_a, first["claim"]["uuid"])
    assert refreshed["claim"]["entry_uuid"] != oldest
    assert claims(app_b)[other["claim"]["uuid"]]["entry_uuid"] == other["claim"]["entry_uuid"]
    reuse = json.loads(json.dumps(ordinary)); reuse["policy_refs"] = reuse_profile["combined_policy_refs"]
    reused = call("acquire", payload("reuse.json", reuse))
    assert reused["reused"]
    # With the provider down, expired discovery fails unless stale results are allowed.
    stale_request = json.loads(json.dumps(reuse)); stale_request["policy_refs"] = stale_profile["combined_policy_refs"]
    stale_path = payload("stale.json", stale_request)
    call("acquire", stale_path)
    state["fail"] = True
    now += DAY
    call("update-claim", app, claim, ok=False)
    stale = call("acquire", stale_path)
    assert stale["stale"], stale
    state["fail"] = False
    # Release retrievals are cached as downloaded assets; no Git checkout is created.
    assert not (ROOT / "source" / source["uuid"] / "repo").exists()
    call("--version", executable=shim, raw=True)
    report = {"root": str(ROOT), "run": str(RUN), "platform": platform, "build_a": {"version": version_a, "sha256": digest(exe_a), "entry": a},
              "build_b": {"version": version_b, "sha256": digest(exe_b), "entry": b}, "app": app, "claim": claim,
              "active": record(app)["active"], "old_process": old_result, "failed_candidate": failure,
              "oldest_reclaimed": oldest, "reacquired": refreshed["claim"]["entry_uuid"], "discovery_checks": state["checks"],
              "root_isolation": "fresh per-run root; all commands explicit root; child root matched; no production-root fallback", "result": "passed"}
    (RUN / "report.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    (REPO / "target" / "acceptance" / "latest.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, indent=2), flush=True)
finally:
    if "old" in globals() and old.poll() is None:
        release.write_text("exit", encoding="utf-8")
        old.communicate(timeout=30)
    server.shutdown()
