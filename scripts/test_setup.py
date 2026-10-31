"""Setup acceptance using actual built binaries, isolated roots and loopback releases.

Run cargo build --bins, then python -m unittest discover -s scripts -p test_setup.py.
"""
import hashlib
import http.server
import io
import json
import os
from pathlib import Path
import platform
import subprocess
import shutil
import time
import tempfile
import threading
import unittest
import uuid
import zipfile
import warnings
from concurrent.futures import ThreadPoolExecutor
import release

REPO = Path(__file__).resolve().parents[1]
EXT = ".exe" if os.name == "nt" else ""
NAME = "saucepan" + EXT
EXE = REPO / "target/debug" / NAME
SHIM = REPO / "target/debug" / ("saucepan-shim" + EXT)
TARGET = {("Windows", "AMD64"): "x86_64-pc-windows-msvc",
          ("Windows", "ARM64"): "aarch64-pc-windows-msvc",
          ("Linux", "x86_64"): "x86_64-unknown-linux-musl",
          ("Darwin", "arm64"): "aarch64-apple-darwin",
          ("Darwin", "x86_64"): "x86_64-apple-darwin"}.get((platform.system(), platform.machine()))


class SetupAcceptance(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not EXE.is_file() or not SHIM.is_file() or TARGET is None:
            raise RuntimeError("build the host binaries with cargo build --bins first")
        archive = io.BytesIO()
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
            for path in (EXE, SHIM, REPO / "LICENSE"):
                output.write(path, path.name)
        cls.archive = archive.getvalue()
        shell_archive = io.BytesIO()
        with zipfile.ZipFile(shell_archive, 'w', zipfile.ZIP_DEFLATED) as output:
            output.write(EXE, 'saucepan')
            output.write(SHIM, 'saucepan-shim')
            output.write(REPO / 'LICENSE', 'LICENSE')
        cls.shell_archive = shell_archive.getvalue()
        cls.requests = []
        cls.events = []
        cls.blocked = threading.Event()
        cls.release = threading.Event()
        cls.block = False
        invalid = io.BytesIO()
        with zipfile.ZipFile(invalid, "w") as output:
            output.writestr(NAME, b"invalid executable")
        cls.invalid = invalid.getvalue()

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                cls.requests.append(self.path)
                if "/releases/assets/" in self.path:
                    data = cls.invalid if self.path.endswith("/2") else cls.archive
                elif self.path.startswith("/repos/"):
                    tag = self.path.rsplit("/", 1)[-1]
                    if cls.block:
                        cls.blocked.set()
                        if not cls.release.wait(20):
                            self.send_error(500)
                            return
                    bad = tag == 'vbad' or tag.endswith('-bad')
                    payload = cls.invalid if bad else cls.archive
                    data = json.dumps({"id": 1, "tag_name": tag, "draft": False, "prerelease": False,
                                       "assets": [{"id": 2 if bad else 1, "name": f"saucepan-{tag}-{TARGET}.zip",
                                                   "browser_download_url": cls.base + ("/invalid" if bad else "/archive"),
                                                   "digest": "sha256:" + hashlib.sha256(payload).hexdigest()}]}).encode()
                elif self.path == "/archive":
                    data = cls.archive
                elif self.path == "/invalid":
                    data = cls.invalid
                elif self.path.startswith('/releases/'):
                    tag, filename = self.path.split('/')[-2:]
                    payload = cls.archive
                    if tag == 'v0.0.0':
                        legacy = io.BytesIO()
                        with zipfile.ZipFile(legacy, 'w') as output:
                            output.write(SHIM, NAME)  # A real executable without the manager setup CLI.
                            output.write(SHIM, SHIM.name)
                            output.write(REPO / 'LICENSE', 'LICENSE')
                        payload = legacy.getvalue()
                    elif tag in ('v0.0.2', 'v0.0.3', 'v0.0.4'):
                        malformed = io.BytesIO()
                        with zipfile.ZipFile(malformed, 'w') as output:
                            output.write(EXE, NAME)
                            with warnings.catch_warnings():
                                warnings.simplefilter('ignore', UserWarning)
                                output.write(SHIM, NAME if tag == 'v0.0.4' else SHIM.name)
                            output.write(REPO / 'LICENSE', 'LICENSE')
                            if tag != 'v0.0.4':
                                output.writestr('../escape' if tag == 'v0.0.2' else 'unexpected', b'not allowed')
                        payload = malformed.getvalue()
                    if filename == 'SHA256SUMS':
                        digest = '0' * 64 if tag == 'v0.0.1' else hashlib.sha256(payload).hexdigest()
                        data = f'{digest}  saucepan-{tag}-{TARGET}.zip\n'.encode()
                        if os.name == 'nt':
                            shell_digest = '0' * 64 if tag == 'v0.0.1' else hashlib.sha256(cls.shell_archive).hexdigest()
                            data += f'{shell_digest}  saucepan-{tag}-x86_64-unknown-linux-musl.zip\n'.encode()
                    else:
                        data = cls.shell_archive if os.name == 'nt' and filename.endswith('x86_64-unknown-linux-musl.zip') else payload
                else:
                    self.send_error(404)
                    return
                try:
                    self.send_response(200)
                    self.send_header("Content-Length", str(len(data)))
                    self.end_headers()
                    self.wfile.write(data)
                except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
                    pass  # A crash-recovery test intentionally terminates the client.

            def log_message(self, *_):
                pass

        cls.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        cls.base = f"http://127.0.0.1:{cls.server.server_port}"
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join()
        evidence = {'platform': platform.platform(), 'binary': str(EXE),
                    'binary_sha256': hashlib.sha256(EXE.read_bytes()).hexdigest(),
                    'shim_sha256': hashlib.sha256(SHIM.read_bytes()).hexdigest(),
                    'requests': cls.requests, 'observations': cls.events}
        report = REPO / 'target/setup-acceptance.json'
        prior = json.loads(report.read_text(encoding='utf-8')) if report.exists() else {'runs': []}
        if 'runs' not in prior:
            prior = {'runs': [prior]}
        prior['runs'].append(evidence)
        report.write_text(json.dumps(prior, indent=2), encoding='utf-8')

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="saucepan setup ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "root space é"

    def call(self, *args, code=0, executable=EXE, raw=False):
        result = subprocess.run([str(executable), "--root", str(self.root), "--test-github-api", self.base,
                                 *map(str, args)], capture_output=True, text=True, encoding="utf-8", timeout=120)
        self.assertEqual(result.returncode, code, (args, result.stdout, result.stderr))
        if raw or code == 1:
            return result
        value = json.loads(result.stdout)
        event = {'command': list(map(str, args)), 'exit_code': code, 'result': value}
        if isinstance(value, dict) and value.get('backup_paths'):
            event['backup_sha256'] = {p: hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in value['backup_paths']}
        self.events.append(event)
        return value

    def test_missing_inspection_and_app_bound_setup_have_no_effects(self):
        self.assertEqual(self.call("setup-status")["state"], "absent")
        self.assertFalse(self.root.exists())
        self.call("--as-app", uuid.uuid4(), "setup", SHIM, "--tag", "v0.11.0", code=1)
        self.assertFalse(self.root.exists())

    def test_fresh_reuse_changed_tag_and_damaged_launcher(self):
        setup = self.call("setup", SHIM, "--tag", "v0.11.0")
        self.assertEqual(setup["status"], "complete")
        self.assertEqual(self.call("setup-status")["state"], "ready")
        launcher = Path(setup["launcher_path"])
        self.assertIn("saucepan ", self.call("--version", executable=launcher, raw=True).stdout)
        requests = len(self.requests)
        before = (self.root / "root.json").read_bytes()
        reused = self.call("setup", SHIM, "--tag", "v0.11.0")
        self.assertFalse(reused["changed"])
        self.assertEqual(reused["app"], setup["app"])
        self.assertEqual(requests, len(self.requests))
        self.assertEqual(before, (self.root / "root.json").read_bytes())
        mismatch = self.call("setup", SHIM, "--tag", "v0.12.0", code=1)
        self.assertIn('self-update', mismatch.stderr)
        self.assertEqual(before, (self.root / "root.json").read_bytes())
        launcher.write_bytes(b"changed")
        self.assertEqual(self.call("setup-status")["state"], "conflict")
        self.call("setup", SHIM, "--tag", "v0.11.0", code=1)
        self.assertEqual(launcher.read_bytes(), b"changed")

    def test_unmanaged_and_unsupported_roots_are_preserved(self):
        self.call("init")
        launcher = self.root / "bin" / NAME
        launcher.write_bytes(b"raw manager")
        self.assertEqual(self.call("setup-status")["state"], "unmanaged")
        self.call("setup", SHIM, "--tag", "v0.11.0", code=1)
        self.assertEqual(launcher.read_bytes(), b"raw manager")
        self.root = Path(self.temp.name) / "unsupported"
        self.root.mkdir()
        (self.root / "existing").write_text("preserve")
        self.call("setup", SHIM, "--tag", "v0.11.0", code=1)
        self.call("setup-status", code=1)
        self.assertEqual(list(p.name for p in self.root.iterdir()), ["existing"])

    def test_explicit_migration_retains_backup_and_rejects_wrong_digest(self):
        self.call("init")
        launcher = self.root / "bin" / NAME
        shutil.copy2(EXE, launcher)
        expected = hashlib.sha256(launcher.read_bytes()).hexdigest()
        self.call("setup", SHIM, "--tag", "v0.11.0", "--migrate-unmanaged",
                  "--expected-sha256", "0" * 64, "--operation", uuid.uuid4(), code=1)
        self.assertEqual(hashlib.sha256(launcher.read_bytes()).hexdigest(), expected)
        operation = str(uuid.uuid4())
        result = self.call("setup", SHIM, "--tag", "v0.11.0", "--migrate-unmanaged",
                           "--expected-sha256", expected, "--operation", operation)
        self.assertEqual(result["status"], "complete")
        self.assertEqual(len(result["backup_paths"]), 1)
        self.assertEqual(hashlib.sha256(Path(result["backup_paths"][0]).read_bytes()).hexdigest(), expected)
        self.assertEqual(self.call("setup-status")["state"], "ready")
        self.assertEqual(hashlib.sha256(launcher.read_bytes()).hexdigest(), hashlib.sha256(SHIM.read_bytes()).hexdigest())

    def test_hardlinks_and_sidecars_are_not_migrated(self):
        self.call("init")
        launcher = self.root / "bin" / NAME
        shutil.copy2(EXE, launcher)
        expected = hashlib.sha256(launcher.read_bytes()).hexdigest()
        alias = Path(self.temp.name) / "hardlink"
        os.link(launcher, alias)
        args = ("setup", SHIM, "--tag", "v0.11.0", "--migrate-unmanaged",
                "--expected-sha256", expected, "--operation", uuid.uuid4())
        self.call(*args, code=1)
        self.assertTrue(launcher.exists())
        alias.unlink()
        sidecar = launcher.with_suffix(".shim.json")
        sidecar.write_bytes(b"unowned sidecar")
        self.call(*args, code=1)
        self.assertEqual(sidecar.read_bytes(), b"unowned sidecar")

    def test_failed_migration_preserves_backup_and_new_attempt_reuses_evidence(self):
        self.call("init")
        launcher = self.root / "bin" / NAME
        shutil.copy2(EXE, launcher)
        expected = hashlib.sha256(launcher.read_bytes()).hexdigest()
        operation = str(uuid.uuid4())
        args = ("setup", SHIM, "--tag", "vbad", "--migrate-unmanaged",
                "--expected-sha256", expected, "--operation", operation)
        failed = self.call(*args, code=2)
        self.assertEqual(failed["status"], "failed")
        self.assertFalse(launcher.exists())
        self.assertEqual(hashlib.sha256(Path(failed["backup_paths"][0]).read_bytes()).hexdigest(), expected)
        before = (self.root / "root.json").read_bytes()
        self.assertEqual(self.call(*args, code=2), failed)
        self.assertEqual(before, (self.root / "root.json").read_bytes())
        good = self.call("setup", SHIM, "--tag", "v0.11.0", "--migrate-unmanaged",
                         "--expected-sha256", expected, "--operation", uuid.uuid4())
        self.assertEqual(good["status"], "complete")
        self.assertEqual(good["backup_paths"], failed["backup_paths"])

    def test_concurrent_setup_reuses_one_application(self):
        self.call("init")
        with ThreadPoolExecutor(max_workers=2) as pool:
            jobs = [pool.submit(self.call, "setup", SHIM, "--tag", "v0.11.0") for _ in range(2)]
            results = [job.result() for job in jobs]
        self.assertEqual(results[0]["app"], results[1]["app"])
        self.assertEqual(sum(result["changed"] for result in results), 1)

    def test_interrupted_preparation_uses_durable_shim_and_original_identities(self):
        cls = type(self)
        cls.blocked.clear()
        cls.release.clear()
        cls.block = True
        operation = str(uuid.uuid4())
        staged = Path(self.temp.name) / ("external shim" + EXT)
        shutil.copy2(SHIM, staged)
        process = subprocess.Popen([str(EXE), "--root", str(self.root), "--test-github-api", self.base,
                                    "setup", str(staged), "--tag", "v0.11.0", "--operation", operation],
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            self.assertTrue(cls.blocked.wait(20), "setup never reached release acquisition")
            process.kill()
            process.communicate(timeout=10)
        finally:
            cls.block = False
            cls.release.set()
            if process.poll() is None:
                process.kill()
                process.communicate(timeout=10)
        staged.unlink()
        self.assertEqual(self.call("setup-status")["operation"], operation)
        durable = self.root / ".staging/setup" / operation / "shim"
        before = (self.root / "root.json").read_bytes()
        sources = sorted(p.name for p in (self.root / "source").iterdir())
        self.call("setup", durable, "--tag", "v0.12.0", "--operation", operation, code=1)
        self.assertEqual(before, (self.root / "root.json").read_bytes())
        result = self.call("setup", durable, "--tag", "v0.11.0", "--operation", operation)
        self.assertEqual(result["status"], "complete")
        self.assertEqual(sources, sorted(p.name for p in (self.root / "source").iterdir()))
        receipt = self.root / ".staging/setup" / operation / "index.json"
        receipt.write_bytes(receipt.read_bytes().replace(b'v0.11.0', b'v99.0.0'))
        self.call("setup-status", code=1)

    def test_explicit_tag_update_changes_claim_and_binding_together(self):
        installed = self.call("setup", SHIM, "--tag", "v0.11.0")
        app, claim = installed["app"], installed["claim"]
        operation = str(uuid.uuid4())
        updated = self.call("self-update", app, claim, "--tag", "v0.12.0", "--operation", operation)
        self.assertEqual(updated["status"], "complete")
        self.assertEqual(self.call("setup-status")["installed_version"], "v0.12.0")
        claims = self.call("claims", app)
        selected = next(item["claim"] for item in claims if item["claim"]["uuid"] == claim)
        self.assertEqual(selected["request"]["selector"], {"type": "tag", "tag": "v0.12.0"})
        self.assertEqual(selected["entry_uuid"], updated["candidate"])
        self.call("self-update", app, claim, "--tag", "v0.13.0", "--operation", operation, code=1)
        failed = self.call("self-update", app, claim, "--tag", "vbad", code=2)
        self.assertEqual(failed["status"], "failed")
        self.assertEqual(self.call("claims", app), claims)
        self.assertEqual(self.call("setup-status")["installed_version"], "v0.12.0")

    def installer(self, version='v0.11.0', code=0, extra=()):
        artifacts = Path(self.temp.name) / 'installer artifacts'
        if not artifacts.exists():
            tag = 'v' + release.tomllib.loads((REPO / 'Cargo.toml').read_text())['package']['version']
            release.installers(tag, artifacts, '0' * 40)  # Explicit local release fixture identity.
        if os.name == 'nt':
            command = ['powershell.exe', '-NoProfile', '-NonInteractive', '-File', str(artifacts / 'install.ps1'),
                       '-Version', version, '-Root', str(self.root), '-TestReleaseBase', self.base + '/releases',
                       '-TestGithubApi', self.base, *map(str, extra)]
        else:
            command = ['sh', str(artifacts / 'install.sh'), '--version', version, '--root', str(self.root),
                       '--test-release-base', self.base + '/releases', '--test-github-api', self.base, *map(str, extra)]
        result = subprocess.run(command, capture_output=True, text=True, encoding='utf-8', timeout=120)
        self.assertEqual(result.returncode, code, (command, result.stdout, result.stderr))
        if code in (0, 2):
            value = json.loads(result.stdout)
            self.events.append({'installer': command, 'exit_code': code, 'result': value})
            return value
        return result

    def test_official_installer_fresh_setup_and_preflight_failures(self):
        for version in ['v0.0.1', 'v0.0.0', 'v0.0.2', 'v0.0.3', 'v0.0.4']:
            self.installer(version, code=1)
            self.assertFalse(self.root.exists())
        result = self.installer()
        self.assertEqual(result['status'], 'complete')
        self.assertEqual(self.call('setup-status')['state'], 'ready')

    def test_official_installer_preserves_noncomplete_receipt(self):
        self.call('init')
        launcher = self.root / 'bin' / NAME
        shutil.copy2(EXE, launcher)
        expected = hashlib.sha256(launcher.read_bytes()).hexdigest()
        operation = str(uuid.uuid4())
        flags = ('-MigrateUnmanaged', '-ExpectedSha256', expected, '-Operation', operation) if os.name == 'nt' else (
            '--migrate-unmanaged', '--expected-sha256', expected, '--operation', operation)
        # The installer archive is valid, but the selected candidate fails its probe.
        result = self.installer('v0.11.0-bad', code=2, extra=flags)
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['operation'], operation)
        self.assertTrue(Path(result['backup_paths'][0]).is_file())

    @unittest.skipUnless(os.name == 'nt' and Path('C:/Program Files/Git/bin/sh.exe').is_file(), 'Git sh fixture runs on Windows only')
    def test_posix_installer_contract_under_git_shell(self):
        # This exercises shell mechanics with real Windows binaries, not native Unix acceptance.
        shell = 'C:/Program Files/Git/bin/sh.exe'
        mock = Path(self.temp.name) / 'commands'
        mock.mkdir()
        (mock / 'uname').write_text('#!/bin/sh\ncase "$1" in -s) echo Linux;; -m) echo x86_64;; esac\n', encoding='utf-8', newline='\n')
        env = dict(os.environ, PATH=str(mock) + os.pathsep + os.environ['PATH'])
        root = subprocess.check_output([shell, '-c', 'cygpath -u "$1"', '--', str(self.root)], text=True, encoding='utf-8').strip()
        mock_path = subprocess.check_output([shell, '-c', 'cygpath -u "$1"', '--', str(mock)], text=True, encoding='utf-8').strip()
        args = [shell, '-c', 'PATH="$1:$PATH"; export PATH; shift; exec sh scripts/install.sh "$@"', '--', mock_path,
                '--root', root, '--operation', str(uuid.uuid4()),
                '--test-release-base', self.base + '/releases', '--test-github-api', self.base]
        bad = subprocess.run([*args, '--version', 'v0.0.1'], env=env, capture_output=True, text=True, encoding='utf-8', timeout=120)
        self.assertEqual(bad.returncode, 1, bad.stderr)
        self.assertIn('checksum mismatch', bad.stderr)
        self.assertFalse(self.root.exists())
        good = subprocess.run([*args, '--version', 'v0.11.0'], env=env, capture_output=True, text=True, encoding='utf-8', timeout=120)
        self.assertEqual(good.returncode, 0, (good.stdout, good.stderr))
        value = json.loads(good.stdout)
        self.assertEqual(value['status'], 'complete')
        self.assertEqual(self.call('setup-status')['state'], 'ready')
        self.events.append({'shell_fixture': 'Git sh with Linux dispatch stub and Windows native binaries', 'result': value})
        before = (self.root / 'root.json').read_bytes()
        (mock / 'uname').write_text('#!/bin/sh\necho unsupported\n', encoding='utf-8', newline='\n')
        unsupported = subprocess.run([*args, '--version', 'v0.11.0'], env=env, capture_output=True, text=True, encoding='utf-8', timeout=20)
        self.assertEqual(unsupported.returncode, 1)
        self.assertIn('Unsupported installer platform', unsupported.stderr)
        minimal = list(args)
        minimal[2] = 'PATH="$1"; export PATH; shift; exec /usr/bin/sh scripts/install.sh "$@"'
        missing = subprocess.run([*minimal, '--version', 'v0.11.0'], env=env, capture_output=True, text=True, encoding='utf-8', timeout=20)
        self.assertEqual(missing.returncode, 1)
        self.assertIn('Missing prerequisite: curl', missing.stderr)
        self.assertEqual(before, (self.root / 'root.json').read_bytes())

    @unittest.skipUnless(os.name == "nt", "Windows executable sharing behavior")
    def test_busy_raw_manager_is_preserved_then_explicitly_retried(self):
        self.call("init")
        launcher = self.root / "bin" / NAME
        shutil.copy2(EXE, launcher)
        expected = hashlib.sha256(launcher.read_bytes()).hexdigest()
        ready, release = Path(self.temp.name) / "ready", Path(self.temp.name) / "release"
        running = subprocess.Popen([str(launcher), "--root", str(self.root), "test-hold", str(ready), str(release)],
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            deadline = time.monotonic() + 10
            while not ready.exists():
                self.assertIsNone(running.poll())
                self.assertLess(time.monotonic(), deadline)
                time.sleep(.01)
            operation = str(uuid.uuid4())
            args = ("setup", SHIM, "--tag", "v0.11.0", "--migrate-unmanaged",
                    "--expected-sha256", expected, "--operation", operation)
            result = self.call(*args, code=2)
            self.assertEqual(result["status"], "preparing")
            self.assertEqual(hashlib.sha256(launcher.read_bytes()).hexdigest(), expected)
            self.assertEqual(self.call("setup-status")["state"], "pending")
        finally:
            release.write_bytes(b"release")
            running.communicate(timeout=15)
        self.assertEqual(self.call(*args)["status"], "complete")


if __name__ == "__main__":
    unittest.main()
