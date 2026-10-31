from pathlib import Path
import sys
import tempfile
import unittest
import zipfile

from acceptance.builds import EXE, Artifacts, Builds, host_target


def archive(path: Path, marker: bytes) -> Path:
    with zipfile.ZipFile(path, "w") as bundle:
        for name in ("saucepan" + EXE, "saucepan-shim" + EXE):
            bundle.writestr(name, marker + name.encode())
        bundle.writestr("LICENSE", b"license")
    return path


class ArtifactsMode(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.candidate, self.twin = self.root / "dist", self.root / "twin"
        self.candidate.mkdir()
        self.twin.mkdir()
        host = host_target()
        archive(self.candidate / f"saucepan-v9.9.9-{host}.zip", b"candidate:")
        other = "aarch64-pc-windows-msvc" if host != "aarch64-pc-windows-msvc" else "x86_64-pc-windows-msvc"
        archive(self.candidate / f"saucepan-v9.9.9-{other}.zip", b"other:")
        archive(self.twin / f"verify-twin-{host}.zip", b"twin:")
        self.other = other
        # A cargo that always fails: any compile attempt breaks the test.
        self.builds = Builds(self.root / "builds", cargo=[sys.executable, "-c", "raise SystemExit(9)"],
                             artifacts=Artifacts(self.candidate, self.twin))

    def test_manager_and_lifecycle_builds_come_from_archives_without_cargo(self):
        manager = self.builds.manager()
        self.assertEqual(manager.manager.read_bytes(), b"candidate:saucepan" + EXE.encode())
        self.assertEqual(manager.build_id, "v9.9.9")
        first, second = self.builds.release("lifecycle-a"), self.builds.release("lifecycle-b")
        self.assertIs(first, manager)
        self.assertEqual(second.manager.read_bytes(), b"twin:saucepan" + EXE.encode())
        self.assertEqual(second.build_id, "v9.9.9-verify")
        self.assertIs(self.builds.release("lifecycle-a"), first)
        with self.assertRaises(RuntimeError):
            self.builds.release("lifecycle-c")
        self.assertEqual(self.builds.invocations, [])

    def test_tag_and_packaged_targets(self):
        artifacts = Artifacts(self.candidate)
        self.assertEqual(artifacts.tag, "v9.9.9")
        self.assertEqual(artifacts.packaged_targets(), sorted({host_target(), self.other}))

    def test_report_records_candidate_previous_and_unverified_targets(self):
        import json
        from acceptance import report
        previous = self.root / "previous"
        previous.mkdir()
        archive(previous / f"saucepan-v9.9.8-{host_target()}.zip", b"previous:")
        artifacts = Artifacts(self.candidate, self.twin, previous)
        builds = Builds(self.root / "report-builds", artifacts=artifacts)
        builds.manager()
        written = report.write(self.root / "run", "run-id", {"s": {"outcome": "passed"}}, builds.built(), artifacts)
        section = json.loads((self.root / "run" / "report.json").read_text())["artifacts"]
        self.assertEqual(section, written["artifacts"])
        self.assertEqual(section["candidate"]["tag"], "v9.9.9")
        self.assertEqual(len(section["candidate"]["archives"]), 2)
        self.assertEqual(section["verified_target"], host_target())
        self.assertEqual(section["unverified_targets"], [self.other])
        self.assertEqual(section["previous"]["tag"], "v9.9.8")
        self.assertIn(f"verify-twin-{host_target()}.zip", section["twin"])

    def test_missing_twin_is_reported(self):
        builds = Builds(self.root / "other", artifacts=Artifacts(self.candidate))
        builds.release("lifecycle-a")
        with self.assertRaisesRegex(RuntimeError, "--twin"):
            builds.release("lifecycle-b")


if __name__ == "__main__":
    unittest.main()
