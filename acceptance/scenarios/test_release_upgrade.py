"""Self-update from the previous published release to the candidate (artifacts mode).

The previous release installs itself through setup and updates to the candidate
through the forge, as an existing managed installation would. The outcome
follows the store contract each release creates: the same contract must keep
working; a changed contract must fail with the documented unsupported_store
reinstall guidance and leave the existing root untouched.
"""
import hashlib
import json
from pathlib import Path
import unittest

from acceptance.builds import EXE, Artifacts, Build, host_target
from acceptance.scenario import Scenario, current_run

NAME = "saucepan" + EXE


def tree(root: Path) -> dict[str, str]:
    """Store data, excluding .locks: each launch creates its own use markers there."""
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob("*")) if p.is_file() and p.relative_to(root).parts[0] != ".locks"}


class ReleaseUpgrade(Scenario):
    tags = frozenset({"release"})

    def run_json(self, *command, code=0):
        result = self.sandbox.launch(*command)
        self.assertEqual(result.returncode, code, (command, result.stdout, result.stderr))
        return result

    def contract(self, build: Build) -> str:
        """The store contract a release writes into a fresh root."""
        root = self.sandbox.dir / f"contract-{build.label}"
        self.run_json(build.manager, "--root", root, "init")
        record = json.loads((root / "root.json").read_text(encoding="utf-8"))
        return record.get("payload", {}).get("contract") or record["contract"]

    def test_previous_release_self_updates_to_candidate(self):
        run = current_run()
        previous = self.builds.previous()
        if run.artifacts is None or previous is None:
            raise unittest.SkipTest("needs --artifacts and --previous")
        candidate = self.builds.manager()
        target = host_target()
        previous_archive, previous_tag = Artifacts.archive(run.artifacts.previous, target)
        candidate_archive, candidate_tag = Artifacts.archive(run.artifacts.candidate, target)
        self.assertNotEqual(previous_tag, candidate_tag, "the previous release must have a different tag")
        for archive, tag in ((previous_archive, previous_tag), (candidate_archive, candidate_tag)):
            self.forge.release("ZackaryW/saucepan", tag, {archive.name: archive.read_bytes()})
        # The previous release downloads through browser URLs, including the candidate it fetches.
        self.forge.browser_downloads = True
        api = ["--test-github-api", self.forge.github_api]
        root = self.sandbox.dir / "managed root"

        installed = json.loads(self.run_json(previous.manager, "--root", root, *api, "setup", previous.shim,
                                             "--tag", previous_tag).stdout)
        self.assertEqual(installed["status"], "complete", installed)
        launcher = root / "bin" / NAME
        self.assertTrue(self.sandbox.launch(launcher, "--version").stdout.strip().endswith("+" + previous.build_id))

        # The installed (previous) manager performs the update, as a user's installation would.
        updated = self.sandbox.launch(launcher, "--root", root, *api, "self-update",
                                      installed["app"], installed["claim"], "--tag", candidate_tag)
        self.assertEqual(updated.returncode, 0, (updated.stdout, updated.stderr))
        version = self.sandbox.launch(launcher, "--version").stdout.strip()
        self.assertTrue(version.endswith("+" + candidate.build_id), version)

        before = tree(root)
        query = self.sandbox.launch(launcher, "--root", root, "--error-format", "json", "query-sources")
        if self.contract(previous) == self.contract(candidate):
            self.assertEqual(query.returncode, 0, query.stderr)
            status = json.loads(self.run_json(launcher, "--root", root, "setup-status").stdout)
            self.assertEqual(status["installed_version"], candidate_tag)
            return
        # A new store contract: the update itself completes (its probe needs no store), but
        # the candidate then refuses the earlier root with the documented reinstall guidance
        # and does not modify it.
        self.assertNotEqual(query.returncode, 0, query.stdout)
        error = json.loads(query.stderr)["error"]
        self.assertEqual(error["code"], "unsupported_store", error)
        self.assertIn("reinstall", error["message"])
        self.assertEqual(tree(root), before, "the candidate modified an earlier-contract root")
