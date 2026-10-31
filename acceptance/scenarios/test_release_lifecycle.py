"""The integrated real-build lifecycle as one coherent sequence (self-management, lifecycle-recovery).

Bootstrap a packaged release build through the fixture, share its output with a
second app, update to distinguishable bytes while the old launcher is busy,
preserve a failed candidate, roll back, deactivate and reactivate, replace the
launcher, uninstall with pending cleanup and reconciliation, and install a
generic tool through the same coordinator.
"""
import hashlib
import os
from pathlib import Path
import time

from acceptance import REPO
from acceptance.builds import EXE, digest
from acceptance.scenario import Scenario, new_uuid, zipped

NAME = "saucepan" + EXE
SOURCE = "fixture/lifecycle"
PROBE = {"args": ["--version"], "stdout_contains": "saucepan ", "timeout_seconds": 10}
WINDOWS = os.name == "nt"


def tree_digest(root: Path) -> dict[str, str]:
    if not root.exists():
        return {}
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob("*")) if p.is_file()}


class ReleaseLifecycle(Scenario):
    tags = frozenset({"lifecycle"})

    def version(self, launcher: Path) -> str:
        result = self.sandbox.launch(launcher, "--version")
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def hold(self, launcher: Path, ready: Path, release: Path):
        for path in (ready, release):
            path.unlink(missing_ok=True)
        process = self.sandbox.background(launcher, "test-hold", ready, release)
        deadline = time.monotonic() + 30
        while not ready.exists():
            self.assertIsNone(process.poll(), "busy launcher exited before it was ready")
            self.assertLess(time.monotonic(), deadline, "busy launcher did not start")
            time.sleep(0.02)
        return process

    def release_hold(self, process, release: Path) -> None:
        release.write_text("release", encoding="utf-8")
        process.communicate(timeout=30)
        self.assertEqual(process.returncode, 0)

    def claim_entry(self, app: str, claim: str) -> str:
        return {v["claim"]["uuid"]: v["claim"] for v in self.client.claims(app)}[claim]["entry_uuid"]

    def test_complete_lifecycle(self):
        mock_before = tree_digest(REPO / "mock")
        build_a = self.builds.release("lifecycle-a")
        build_b = self.builds.release("lifecycle-b")
        self.assertNotEqual(build_a.digest, build_b.digest)
        root, client = self.sandbox.root, self.client

        source = client.register_source({"type": "github_release", "repo": SOURCE})["uuid"]
        keep_two = {"uuid": new_uuid(), "type": "revision_policy", "history": "latest", "keep": 2}
        refs = self.policy_refs("explicit-update", for_install=True, acquisition=[keep_two])
        request = self.release_request(source, refs, self.asset_filter("saucepan.zip"), bin=[NAME])
        self.forge.release(SOURCE, "v1", {"saucepan.zip": build_a.archive.read_bytes()})

        # Bootstrap through the fixture and launch through the stable shim.
        boot = client.bootstrap(request, str(build_a.shim), operation=new_uuid())
        self.assertEqual(boot["exit_code"], 0, boot)
        app, claim, a = boot["result"]["app"], boot["result"]["claim"], boot["result"]["candidate"]
        launcher = root / "bin" / NAME
        self.assertTrue(self.version(launcher).endswith("+lifecycle-a"))
        self.assertEqual(digest(root / "store" / a / NAME), build_a.digest)

        # A second app shares the same output through its own shim.
        other = client.register_application({"registration_key": "shared", "name": "Shared"})["uuid"]
        shared = client.acquire(request, app=other, scope="acceptance/shared")
        self.assertEqual(shared["claim"]["entry_uuid"], a)
        self.admin.activate(other, a, NAME, PROBE)
        other_shim = root / "bin" / ("shared" + EXE)
        self.admin.install_shim(other, other_shim.name, build_b.shim, saucepan=True)

        # Update while the old launcher is busy.
        ready, release = self.sandbox.dir / "ready", self.sandbox.dir / "release"
        running = self.hold(launcher, ready, release)
        self.assertEqual(ready.read_text(), "lifecycle-a")
        self.forge.release(SOURCE, "v2", {"saucepan.zip": build_b.archive.read_bytes()})
        through_launcher = self.sandbox.client_for(launcher)
        update = through_launcher.self_update(app, claim)
        b = update["result"]["candidate"]
        self.assertEqual((update["exit_code"], update["result"]["status"]), (0, "complete"))
        self.assertNotEqual(a, b)
        self.assertEqual(digest(root / "store" / b / NAME), build_b.digest)
        self.assertTrue(self.version(launcher).endswith("+lifecycle-b"))

        # A failing release leaves the selected claim and binding untouched.
        previous = client.inspect(app)
        self.forge.release(SOURCE, "v3", {"saucepan.zip": zipped(**{NAME: b"not executable"})})
        failed = through_launcher.self_update(app, claim)
        self.assertEqual((failed["exit_code"], failed["result"]["status"]), (2, "failed"))
        self.assertEqual(client.inspect(app), previous)

        # Rollback, deactivate and reactivate.
        rollback = client.rollback(app, claim, a, PROBE)
        self.assertEqual(rollback["result"]["status"], "complete")
        self.assertEqual(self.claim_entry(app, claim), a)
        self.admin.deactivate(app)
        rejected = self.sandbox.launch(launcher, "--version")
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("no active binding", rejected.stderr)
        self.admin.activate(app, a, NAME, PROBE)
        self.assertTrue(self.version(launcher).endswith("+lifecycle-a"))

        # Replace the busy launcher; Windows defers the swap until reconciliation.
        replacement = self.admin.replace_shim(app, NAME, build_b.shim, saucepan=True, code=2 if WINDOWS else 0)
        if WINDOWS:
            self.assertEqual(replacement["status"], "commit_intent")
            self.release_hold(running, release)
            reconciled = client.reconcile(operation=replacement["operation"])
            self.assertEqual(reconciled["result"][0]["status"], "complete")
            self.assertEqual(digest(launcher), digest(build_b.shim))
            running = self.hold(launcher, ready, release)

        # Uninstall while busy: cleanup stays pending; shared output stays protected.
        removed = through_launcher.uninstall(app)
        self.assertEqual(removed["result"]["status"], "cleanup_pending" if WINDOWS else "complete")
        self.assertIsNone(running.poll())
        self.assertTrue(self.version(other_shim).endswith("+lifecycle-a"))
        self.assertIn(a, client.reclaim()["deferred"])
        self.assertEqual(client.uninstall(other)["result"]["status"], "complete")
        self.assertIn(a, client.reclaim()["deferred"])
        self.release_hold(running, release)
        done = client.reconcile(operation=removed["result"]["operation"])
        self.assertEqual(done["result"][0]["status"], "complete")
        self.assertEqual(client.reconcile(operation=removed["result"]["operation"])["result"][0],
                         done["result"][0])
        self.assertFalse(launcher.exists())
        self.assertFalse((root / "index" / app).exists())
        self.assertIn(a, client.reclaim()["reclaimed"])
        self.assertEqual(client.inspect(a)["state"], "reclaimed")
        self.assertEqual(client.inspect(b)["state"], "ready")
        self.assertFalse((root / "source" / source / "repo").exists())

        # Generic installation through the same coordinator.
        local = client.register_source({"type": "local", "path": str(build_b.manager)})["uuid"]
        generic_request = {"source_uuid": local, "selector": {"type": "local"},
                           "selection": {"bin": [NAME]}, "policy_refs": refs}
        generic = client.install_application("generic", generic_request, PROBE, str(build_b.shim),
                                             scope="acceptance/generic", shim="generic" + EXE)
        self.assertEqual(generic["result"]["status"], "complete")
        self.assertTrue(self.version(root / "bin" / ("generic" + EXE)).endswith("+lifecycle-b"))
        self.assertEqual(client.uninstall(generic["result"]["app"])["result"]["status"], "complete")
        self.assertEqual(tree_digest(REPO / "mock"), mock_before, "historical mock changed")
