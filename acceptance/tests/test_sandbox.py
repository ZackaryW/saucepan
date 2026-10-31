import os
from pathlib import Path
import tempfile
import unittest

from acceptance.builds import ORIGINAL_ENV
from acceptance.sandbox import Run, Sandbox


def snapshot(path: Path):
    if not path.exists():
        return None
    return sorted((str(p.relative_to(path)), p.stat().st_mtime_ns) for p in path.rglob("*"))


class SandboxIsolation(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.run = Run(Path(self.temp.name) / "run")
        self.run.builds.manager()
        self.real_root = Path(ORIGINAL_ENV.get("USERPROFILE" if os.name == "nt" else "HOME", "")) / ".saucepan"
        self.before = snapshot(self.real_root)
        self.run.isolate()

    def tearDown(self):
        self.run.restore()
        self.temp.cleanup()

    def test_default_root_resolves_inside_the_run_home(self):
        sandbox = Sandbox(self.run, "default-root")
        self.addCleanup(sandbox.close)
        created = sandbox.cli("init", root=False)
        self.assertTrue(os.path.samefile(created["root"], sandbox.home / ".saucepan"), created)
        self.assertEqual(snapshot(self.real_root), self.before)

    def test_scenarios_get_separate_initialized_roots(self):
        first, second = Sandbox(self.run, "first"), Sandbox(self.run, "second")
        self.addCleanup(first.close)
        self.addCleanup(second.close)
        first.client.register_source({"type": "local", "path": str(first.dir)})
        self.assertNotEqual(first.root, second.root)
        self.assertEqual(second.client.query_sources()["sources"], [])
        self.assertTrue(second.root.joinpath("root.json").exists())
        self.assertNotEqual(first.home, second.home)

    def test_clock_and_endpoints_reach_the_manager(self):
        sandbox = Sandbox(self.run, "endpoints")
        self.addCleanup(sandbox.close)
        sandbox.forge.release("fixture/tool", "v1", {"tool.bin": b"payload"})
        source = sandbox.client.register_source({"type": "github_release", "repo": "fixture/tool"})
        sandbox.clock.advance(days=2)
        self.assertIn(str(100 + 2 * 86400), sandbox.global_args())
        self.assertIn("--test-github-raw", sandbox.global_args())
        self.assertTrue(source["uuid"])


if __name__ == "__main__":
    unittest.main()
