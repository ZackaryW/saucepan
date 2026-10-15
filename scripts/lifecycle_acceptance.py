"""Run the complete lifecycle with real release builds in a fresh isolated root."""
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
RUN = REPO / "target" / "lifecycle-acceptance" / str(uuid.uuid4())
ROOT = RUN / "root"
EXT = ".exe" if os.name == "nt" else ""
NAME = "saucepan" + EXT
RUN.mkdir(parents=True)

def digest(path):
    with open(path, "rb") as f:
        return hashlib.file_digest(f, "sha256").hexdigest()

def snapshot_mock():
    root = REPO / "mock"
    return {str(p.relative_to(root)): digest(p) for p in sorted(root.rglob("*")) if p.is_file()}

before_mock = snapshot_mock()
def build(label):
    subprocess.run(["cargo", "build", "--locked", "--release", "--bins"], cwd=REPO,
                   env=dict(os.environ, SAUCEPAN_BUILD_ID=label), check=True)
    directory = RUN / label
    directory.mkdir()
    binary = directory / NAME
    launcher = directory / ("saucepan-shim" + EXT)
    shutil.copy2(REPO / "target/release" / NAME, binary)
    shutil.copy2(REPO / "target/release" / launcher.name, launcher)
    archive = directory / "saucepan.zip"
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as z:
        z.write(binary, NAME)
    return binary, launcher, archive

print("Building lifecycle A and B", flush=True)
exe_a, shim_a, zip_a = build("lifecycle-a")
exe_b, shim_b, zip_b = build("lifecycle-b")
assert digest(exe_a) != digest(exe_b)
bad = RUN / "bad.zip"
with zipfile.ZipFile(bad, "w") as z:
    z.writestr(NAME, b"not executable")
archives = {1: zip_a, 2: zip_b, 3: bad}
state = {"version": 1}
class Fixture(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path.startswith("/repos/"):
            n = state["version"]
            data = json.dumps({"id": n, "tag_name": f"v{n}", "draft": False, "prerelease": False,
                "assets": [{"id": n, "name": "saucepan.zip", "browser_download_url": f"{base}/asset/{n}",
                            "digest": "sha256:" + digest(archives[n])}]}).encode()
        elif self.path.startswith("/asset/"):
            data = archives[int(self.path.rsplit("/", 1)[1])].read_bytes()
        else:
            self.send_error(404)
            return
        self.send_response(200)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def log_message(self, *_):
        pass
server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
base = f"http://127.0.0.1:{server.server_port}"
threading.Thread(target=server.serve_forever, daemon=True).start()
events = []
def call(*args, executable=exe_b, code=0, raw=False):
    command = [str(executable), "--root", str(ROOT), "--test-github-api", base, "--test-now", "100"]
    result = subprocess.run(command + list(map(str, args)), cwd=REPO, capture_output=True, text=True, timeout=90)
    assert result.returncode == code, (args, result.returncode, result.stdout, result.stderr)
    if raw:
        return result.stdout.strip()
    value = json.loads(result.stdout)
    events.append({"command": list(map(str, args)), "code": result.returncode, "result": value})
    return value
def save(name, value):
    path = RUN / name
    path.write_text(json.dumps(value, indent=2), encoding="utf-8")
    return path
def wait_file(path, process):
    end = time.monotonic() + 20
    while not path.exists():
        assert process.poll() is None and time.monotonic() < end, f"target did not start: {path}"
        time.sleep(.02)
def inspect(app):
    return call("inspect", app)
running = None
release = RUN / "release"
try:
    initialized = call("init")
    assert initialized["integrity"] == "basic"
    source = call("register-source", save("source.json", {"type": "github_release", "repo": "fixture/lifecycle"}))["uuid"]
    revision = {"uuid": str(uuid.uuid4()), "type": "revision_policy", "history": "latest", "keep": 2}
    profile = call("profile", "explicit-update", "--for-install", "--catalog", save("custom.json", [revision]),
                   "--acquisition-policy", revision["uuid"])
    assert profile == call("profile", "explicit-update", "--for-install", "--catalog", RUN / "custom.json",
                           "--acquisition-policy", revision["uuid"])
    asset_filter = {"uuid": str(uuid.uuid4()), "type": "github_release_asset_filter", "asset_patterns": ["saucepan.zip"]}
    call("register-policies", save("policies.json", profile["catalog"] + [asset_filter]))
    request = {"source_uuid": source, "selector": {"type": "latest_stable"},
               "selection": {"format": "zip", "bin": [NAME], "filter_refs": [asset_filter["uuid"]]},
               "policy_refs": profile["combined_policy_refs"]}
    request_path = save("request.json", request)
    probe = save("probe.json", {"args": ["--version"], "stdout_contains": "saucepan ", "timeout_seconds": 10})
    boot = call("bootstrap", request_path, shim_a, "--operation", uuid.uuid4())
    app, claim, a = boot["app"], boot["claim"], boot["candidate"]
    assert boot["status"] == "complete"
    launcher = ROOT / "bin" / NAME
    assert call("--version", executable=launcher, raw=True).endswith("+lifecycle-a")
    assert digest(ROOT / "store" / a / NAME) == digest(exe_a)

    other = call("register-app", "shared")["uuid"]
    shared = call("acquire", request_path, "--app", other, "--scope", "acceptance/shared")
    assert shared["claim"]["entry_uuid"] == a
    call("activate", other, a, NAME, probe)
    other_name = "shared" + EXT
    call("install-shim", other, other_name, shim_b, "--saucepan")

    ready = RUN / "ready"
    running = subprocess.Popen([str(launcher), "test-hold", str(ready), str(release)],
                               cwd=REPO, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    wait_file(ready, running)
    assert ready.read_text() == "lifecycle-a"
    state["version"] = 2
    update = call("self-update", app, claim, executable=launcher)
    b = update["candidate"]
    assert b != a and update["status"] == "complete"
    assert digest(ROOT / "store" / b / NAME) == digest(exe_b)
    assert call("--version", executable=launcher, raw=True).endswith("+lifecycle-b")

    previous = inspect(app)
    state["version"] = 3
    failed = call("self-update", app, claim, executable=launcher, code=2)
    assert failed["status"] == "failed" and inspect(app) == previous
    rollback = call("rollback", app, claim, a, probe)
    current_claims = {view["claim"]["uuid"]: view["claim"] for view in call("claims", app)}
    assert rollback["status"] == "complete" and current_claims[claim]["entry_uuid"] == a
    call("deactivate", app)
    rejected = subprocess.run([str(launcher), "--version"], capture_output=True, text=True, timeout=10)
    assert rejected.returncode != 0 and "no active binding" in rejected.stderr
    call("activate", app, a, NAME, probe)
    assert call("--version", executable=launcher, raw=True).endswith("+lifecycle-a")

    replacement = call("replace-shim", app, NAME, shim_b, "--saucepan", code=2 if os.name == "nt" else 0)
    if os.name == "nt":
        assert replacement["status"] == "commit_intent"
        release.write_text("release", encoding="utf-8")
        running.communicate(timeout=15)
        assert running.returncode == 0
        running = None
        assert call("reconcile", "--operation", replacement["operation"])[0]["status"] == "complete"
        assert digest(launcher) == digest(shim_b)
        # Hold the upgraded launcher during self-uninstall.
        ready.unlink()
        release.unlink()
        running = subprocess.Popen([str(launcher), "test-hold", str(ready), str(release)],
                                   cwd=REPO, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        wait_file(ready, running)

    removed = call("uninstall", app, executable=launcher, code=2 if os.name == "nt" else 0)
    assert removed["status"] == ("cleanup_pending" if os.name == "nt" else "complete")
    assert running.poll() is None
    assert call("--version", executable=ROOT / "bin" / other_name, raw=True).endswith("+lifecycle-a")
    assert a in call("reclaim")["deferred"]
    assert call("uninstall", other)["status"] == "complete"
    assert a in call("reclaim")["deferred"]
    release.write_text("release", encoding="utf-8")
    running.communicate(timeout=15)
    assert running.returncode == 0
    running = None
    reconciled = call("reconcile", "--operation", removed["operation"])[0]
    assert reconciled["status"] == "complete"
    # The app session is revoked after retirement; retry its durable receipt
    # through the explicit administrative reconciliation boundary.
    assert call("reconcile", "--operation", removed["operation"])[0] == reconciled
    assert not launcher.exists() and not (ROOT / "index" / app).exists()
    assert a in call("reclaim")["reclaimed"]
    assert inspect(a)["state"] == "reclaimed" and inspect(b)["state"] == "ready"
    assert (ROOT / "source" / f"{source}.json").exists()
    assert not (ROOT / "source" / source / "repo").exists()

    # Exercise generic installation through the same coordinator.
    local = call("register-source", save("local.json", {"type": "local", "path": str(exe_b)}))["uuid"]
    generic_request = save("generic.json", {"source_uuid": local, "selector": {"type": "local"},
        "selection": {"bin": [NAME]}, "policy_refs": profile["combined_policy_refs"]})
    generic = call("install", "--scope", "acceptance/generic", "generic", generic_request, probe, shim_b, "--shim", "generic" + EXT)
    assert generic["status"] == "complete"
    assert call("--version", executable=ROOT / "bin" / ("generic" + EXT), raw=True).endswith("+lifecycle-b")
    assert call("uninstall", generic["app"])["status"] == "complete"
    assert before_mock == snapshot_mock()
    report = {"root": str(ROOT), "host": os.name, "integrity": "basic", "native_keyring": "not_used", "build_a": digest(exe_a), "build_b": digest(exe_b),
              "shim_a": digest(shim_a), "shim_b": digest(shim_b), "app": app, "claim": claim,
              "entry_a": a, "entry_b": b, "removal": reconciled, "mock_unchanged": True,
              "mock_file_count": len(before_mock), "events": events}
    report_path = save("report.json", report)
    (RUN.parent / "latest.json").write_text(json.dumps({"report": str(report_path)}, indent=2), encoding="utf-8")
    print(json.dumps({"report": str(report_path), "mock_unchanged": True, "events": len(events)}), flush=True)
finally:
    if running is not None:
        release.write_text("release", encoding="utf-8")
        running.communicate(timeout=15)
    server.shutdown()
    assert before_mock == snapshot_mock(), "mock changed during isolated acceptance"
