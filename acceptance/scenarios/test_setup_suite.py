"""Managed setup and installer acceptance (scripts/test_setup.py) under the ``setup`` tag.

The suite keeps its own explicit roots and installer flows and shares the forge.
From source it runs the debug build; in artifacts mode it runs the candidate's
packaged binaries and the candidate's own installer scripts, and additionally
installs the candidate tag from its real archive and SHA256SUMS.
"""
from pathlib import Path
import sys
import unittest

from acceptance.builds import Artifacts, host_target
from acceptance.forge import Forge
from acceptance.scenario import current_run

_SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
if str(_SCRIPTS) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS))

import test_setup as _setup  # noqa: E402  (imported as a module so the base class is not collected twice)


class ManagedSetup(_setup.SetupAcceptance):
    tags = frozenset({"setup"})

    @classmethod
    def setUpClass(cls):
        run = current_run()
        build = run.builds.manager()
        cls.exe, cls.shim = build.manager, build.shim
        if run.artifacts is not None:
            cls.installers_dir = run.artifacts.candidate
        super().setUpClass()
        cls.artifacts = run.artifacts

    def test_candidate_installs_from_its_own_release_assets(self):
        if self.artifacts is None:
            raise unittest.SkipTest("artifacts mode only")
        archive, tag = Artifacts.archive(self.artifacts.candidate, host_target())
        # A forge of its own: the candidate tag may equal a fabricated fixture tag.
        forge = Forge().start()
        self.addCleanup(forge.close)
        forge.release("ZackaryW/saucepan", tag, {archive.name: archive.read_bytes()})
        forge.installer(tag, {archive.name: archive.read_bytes(),
                              "SHA256SUMS": (self.artifacts.candidate / "SHA256SUMS").read_bytes()})
        result = self.installer(tag, base=forge.base)
        self.assertEqual(result["status"], "complete")
        status = self.call("setup-status")
        self.assertEqual((status["state"], status["installed_version"]), ("ready", tag))
