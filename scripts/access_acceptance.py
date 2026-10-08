"""Exercise fresh built binaries under target/, without changing mock or production roots."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
import uuid

REPO = Path(__file__).resolve().parents[1]
SUFFIX = ".exe" if os.name == "nt" else ""
RUN = REPO / "target" / "access-acceptance" / str(uuid.uuid4())
RUN.mkdir(parents=True)
ROOT = RUN / "root"


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def mock_launcher_state():
    directory = REPO / "mock" / "bin"
    return {p.name: digest(p) for p in directory.iterdir() if p.is_file()} if directory.exists() else {}


def execute(args, **kwargs):
    result = subprocess.run(list(map(str, args)), cwd=REPO, capture_output=True, text=True, **kwargs)
    if result.returncode:
        raise RuntimeError(f"{args}: {result.stdout}\n{result.stderr}")
    return result


before = mock_launcher_state()
build_env = dict(os.environ, SAUCEPAN_BUILD_ID="efficient-access")
execute(["cargo", "build", "--release", "--bins", "--examples"], env=build_env)
BINARY = REPO / "target" / "release" / ("saucepan" + SUFFIX)
SHIM = REPO / "target" / "release" / ("saucepan-shim" + SUFFIX)
CONSUMER = REPO / "target" / "release" / "examples" / ("read-resource" + SUFFIX)


def call(*args):
    return json.loads(execute([BINARY, "--root", ROOT, *args]).stdout)


def save(name, value):
    path = RUN / name
    path.write_text(json.dumps(value), encoding="utf-8")
    return path


call("init")
app = call("register-app", "fresh-managed-saucepan")["uuid"]
source = call("register-source", save("app-source.json", {"type": "local", "path": str(BINARY)}))["uuid"]
request = save("app-request.json", {
    "source_uuid": source, "selector": {"type": "local"},
    "selection": {"bin": [BINARY.name]},
    "policies": {"protection": {"type": "useable_app"}, "identical": "new_entry", "eviction": {"strategy": "latest", "keep": 1}},
})
managed = call("acquire", request, "--app", app)
entry = managed["claim"]["entry_uuid"]
for _ in range(5):
    assert call("path", entry)["verification"] == "metadata"
    assert call("verify", entry)["verification"] == "full"
lookup_markers = len(list((ROOT / ".locks").glob("live-*")))
assert lookup_markers == 0
probe = save("probe.json", {"args": ["--version"], "stdout_contains": "saucepan 0.1.0+efficient-access", "timeout_seconds": 10})
call("activate", app, entry, BINARY.name, probe)
installed = ROOT / "bin" / call("install-shim", app, BINARY.name, SHIM, "--saucepan")["shims"][0]["desired"]["name"]
version = execute([installed, "--version"]).stdout.strip()
assert version == "saucepan 0.1.0+efficient-access"
assert digest(installed) == digest(SHIM)
assert call("path", entry)["verification"] == "metadata"
assert call("verify", entry)["verification"] == "full"
# A new payload file is deliberately invisible to routine launch, but detected by full verification.
extra = Path(managed["acquired"]["directory"]) / "unexpected-data"
extra.write_text("detect me", encoding="utf-8")
try:
    assert execute([installed, "--version"]).stdout.strip() == version
    failure = subprocess.run([str(BINARY), "--root", str(ROOT), "verify", entry], capture_output=True, text=True)
    assert failure.returncode and not failure.stdout and "integrity mismatch" in failure.stderr
finally:
    extra.unlink()

# Copy an existing launcher into this isolated installation. Its marker format
# must remain understandable to the newly built reclaimer after wrapper death.
old_shim = REPO / "mock" / "bin" / ("saucepan" + SUFFIX)
old_marker_verified = False
if os.name == "nt" and old_shim.exists():
    legacy = ROOT / "bin" / call("install-shim", app, "previous-launcher" + SUFFIX, old_shim, "--saucepan")["shims"][0]["desired"]["name"]
    # Start the legacy reader against its original record shape, then upgrade
    # metadata after its lease exists. Old writers against new records are unsupported.
    app_record = ROOT / "index" / app / "index.json"
    current_record = app_record.read_bytes()
    legacy_record = json.loads(current_record)
    for field in ("revision", "state", "shims"):
        legacy_record.pop(field, None)
    app_record.write_text(json.dumps(legacy_record), encoding="utf-8")
    ready_old, release_old = RUN / "old-ready", RUN / "old-release"
    running = subprocess.Popen([str(legacy), "test-hold", str(ready_old), str(release_old)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    try:
        deadline = time.monotonic() + 10
        while not ready_old.exists():
            assert time.monotonic() < deadline and running.poll() is None
            time.sleep(.01)
        app_record.write_bytes(current_record)
        replacement = call("acquire", request, "--app", app)["claim"]["entry_uuid"]
        call("activate", app, replacement, BINARY.name, probe)
        running.kill()
        running.wait(timeout=10)
        assert call("reclaim")["deferred"][entry] == "inherited execution lifetime"
        old_marker_verified = True
    finally:
        if app_record.read_bytes() != current_record and "revision" not in json.loads(app_record.read_bytes()):
            app_record.write_bytes(current_record)
        release_old.write_text("release", encoding="utf-8")
        running.communicate(timeout=10)
    deadline = time.monotonic() + 10
    while entry not in call("reclaim")["reclaimed"]:
        assert time.monotonic() < deadline
        time.sleep(.01)
    entry = replacement

# Closed markers from older lookups are cleaned even for the active output.
stale_markers = [ROOT / ".locks" / f"live-{entry}-{uuid.uuid4()}" for _ in range(5)]
for marker in stale_markers:
    marker.touch()
call("reclaim")
assert all(not marker.exists() for marker in stale_markers)
assert call("path", entry)["entry_uuid"] == entry

(RUN / "data").write_text("leased payload", encoding="utf-8")
data_source = call("register-source", save("data-source.json", {"type": "local", "path": str(RUN / "data")}))["uuid"]
data_request = save("data-request.json", {
    "source_uuid": data_source, "selector": {"type": "local"},
    "policies": {"identical": "new_entry", "eviction": {"strategy": "latest", "keep": 1}},
})
owner = call("register-app", "data-consumer")["uuid"]
data = call("acquire", data_request, "--app", owner)
data_entry = data["claim"]["entry_uuid"]
ready, release = RUN / "ready", RUN / "release"
child = subprocess.Popen(
    [str(BINARY), "--root", str(ROOT), "run", data_entry, "--verification", "full", "--", str(CONSUMER)],
    env=dict(os.environ, CONSUMER_READY=str(ready), CONSUMER_RELEASE=str(release)),
    stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
)
try:
    deadline = time.monotonic() + 10
    while not ready.exists():
        assert time.monotonic() < deadline and child.poll() is None, "consumer did not start"
        time.sleep(.01)
    call("release-claim", owner, data["claim"]["uuid"])
    assert data_entry in call("acquire", data_request)["reclaim"]["deferred"]
    release.write_text("release", encoding="utf-8")
    stdout, stderr = child.communicate(timeout=10)
    assert child.returncode == 0 and stdout == "leased payload" and not stderr
    assert data_entry in call("reclaim")["reclaimed"]
finally:
    if child.poll() is None:
        release.write_text("release", encoding="utf-8")
        child.communicate(timeout=10)

# Observe actual inspection IO at the same access and pre-spawn boundaries.
io = execute(["cargo", "test", "--lib", "core::store::access::tests", "--", "--nocapture"])
io_text = io.stdout + io.stderr
assert "metadata=0 directories/0 opens/0 bytes; full=1 directory/1 open/7 bytes" in io_text
assert "managed launch boundary payload IO: 0 directories/0 opens/0 bytes" in io_text
(RUN / "payload-io.log").write_text(io_text, encoding="utf-8")
native = execute(["cargo", "test", "--test", "access", "claim_selection", "--", "--nocapture"])
(RUN / "native-claim.log").write_text(native.stdout + native.stderr, encoding="utf-8")
after = mock_launcher_state()
assert before == after, "existing mock launcher changed"
report = {
    "root": str(ROOT), "version": version, "managed_entry": entry, "reclaimed_data_entry": data_entry,
    "sha256": {p.name: digest(p) for p in [BINARY, SHIM, CONSUMER]},
    "installed_shim_sha256": digest(installed),
    "payload_io_evidence": str(RUN / "payload-io.log"),
    "native_claim_evidence": str(RUN / "native-claim.log"),
    "existing_mock_launcher_unchanged": before == after,
    "previous_launcher_marker_compatible_after_wrapper_death": old_marker_verified,
    "markers_after_five_path_and_verify_calls": lookup_markers,
    "stale_markers_removed_from_active_output": len(stale_markers),
    "platform": os.name,
}
save("report.json", report)
(RUN.parent / "latest.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
print(json.dumps(report, indent=2))
