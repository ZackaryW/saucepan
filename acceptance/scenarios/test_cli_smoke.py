"""Literal-argv checks the SDK would hide: parsing, roots, error envelope, exit codes."""
import json
import os

from acceptance.scenario import Scenario


class CliSmoke(Scenario):
    tags = frozenset({"cli"})

    def test_help_needs_no_store(self):
        missing = self.sandbox.dir / "never-created"
        result = self.sandbox.launch(self.sandbox.manager, "--root", missing, "--help")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Usage", result.stdout)
        self.assertNotIn("--test-", result.stdout)
        self.assertFalse(missing.exists())

    def test_explicit_root_takes_precedence_over_default(self):
        source = self.client.register_source({"type": "local", "path": str(self.sandbox.dir)})
        self.client.set_source_visibility(source["uuid"], "public")
        listed = self.sandbox.cli("query-sources", "--uuid", source["uuid"])
        self.assertEqual([view["uuid"] for view in listed["sources"]], [source["uuid"]])
        default = self.sandbox.cli("--error-format", "json", "query-sources", root=False, code=1, raw=True)
        self.assertEqual(json.loads(default.stderr)["error"]["code"], "unsupported_store")

    def test_error_envelope_and_exit_code(self):
        bad = self.sandbox.payload("removed-type", {"type": "git", "origin": "https://example.com/a.git"})
        result = self.sandbox.cli("--error-format", "json", "register-source", bad, code=1, raw=True)
        self.assertEqual(result.stdout, "")
        error = json.loads(result.stderr)["error"]
        self.assertEqual(error["code"], "invalid_request")
        self.assertIn("git_repo", error["message"])
        text = self.sandbox.cli("register-source", bad, code=1, raw=True)
        self.assertIn("git_repo", text.stderr)
        self.assertFalse(text.stderr.lstrip().startswith("{"))

    def test_default_root_resolves_inside_sandbox_home(self):
        created = self.sandbox.cli("init", root=False)
        self.assertTrue(os.path.samefile(created["root"], self.sandbox.home / ".saucepan"), created)
