import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

from acceptance.builds import Builds
from acceptance.watchdog import Stall, Watchdog


class BuildsAreMemoized(unittest.TestCase):
    def test_same_label_builds_once(self):
        with tempfile.TemporaryDirectory() as directory:
            # A stand-in cargo that succeeds without building; release() then copies
            # whatever release binaries the checkout already has.
            builds = Builds(Path(directory), cargo=[sys.executable, "-c", "pass"])
            release = Path(__file__).resolve().parents[2] / "target" / "release"
            exe = ".exe" if os.name == "nt" else ""
            if not (release / ("saucepan" + exe)).exists():
                self.skipTest("no release build in target/release")
            first = builds.release("label-a")
            second = builds.release("label-a")
            self.assertIs(first, second)
            self.assertEqual(len(builds.invocations), 1)
            self.assertTrue(first.archive.exists())
            command, label = builds.invocations[0]
            self.assertEqual((command[-2:], label), (["--release", "--bins"], "label-a"))


class WatchdogDiagnosesStalls(unittest.TestCase):
    def test_stalled_child_is_listed_killed_and_next_step_runs(self):
        watchdog = Watchdog()
        sleeper = [sys.executable, "-c", "import time; time.sleep(60)"]
        start = time.monotonic()
        with self.assertRaises(Stall) as caught:
            with watchdog.step("sleeping child", deadline=2):
                subprocess.run(sleeper, timeout=90)
        report = caught.exception.report
        self.assertLess(time.monotonic() - start, 30)
        self.assertEqual(report["label"], "sleeping child")
        self.assertTrue(any("time.sleep(60)" in p["command"] for p in report["processes"]), report)
        self.assertIn("sleeping child stalled", str(caught.exception))
        with watchdog.step("next step", deadline=30):
            subprocess.run([sys.executable, "-c", "pass"], check=True)

    def test_fast_step_passes_through_errors(self):
        with self.assertRaises(ValueError):
            with Watchdog().step("raises", deadline=30):
                raise ValueError("ordinary failure")


if __name__ == "__main__":
    unittest.main()
