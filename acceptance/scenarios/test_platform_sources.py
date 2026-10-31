"""github_raw, github_source and git_raw through the real binary, with provider credentials."""
from pathlib import Path

from acceptance.scenario import Scenario

SKILLS = "owner/skills"
FILES = {"skills/review/SKILL.md": b"review skill", "skills/review/notes.md": b"notes",
         "README.md": b"readme"}


def ref(name):
    return {"type": "ref", "reference": name}


class PlatformSources(Scenario):
    tags = frozenset({"platform-sources"})

    def acquire(self, source: dict, selector: dict, **selection) -> dict:
        uuid = self.client.register_source(source)["uuid"]
        return self.client.acquire({"source_uuid": uuid, "selector": selector,
                                    "selection": selection, "policy_refs": self.policy_refs()})

    def test_github_raw_by_ref_and_pinned_commit_without_gh(self):
        commit = self.forge.commit(SKILLS, FILES, "main")
        by_ref = self.acquire({"type": "github_raw", "repo": SKILLS, "path": "skills/review/SKILL.md"}, ref("main"))
        self.assertEqual(Path(by_ref["directory"], "SKILL.md").read_bytes(), b"review skill")
        resolutions = self.forge.requested(f"/repos/{SKILLS}/commits/")
        self.assertEqual(len(resolutions), 1)
        pinned = self.acquire({"type": "github_raw", "repo": SKILLS, "path": "README.md"},
                              {"type": "commit", "commit": commit})
        self.assertEqual(Path(pinned["directory"], "README.md").read_bytes(), b"readme")
        self.assertEqual(len(self.forge.requested(f"/repos/{SKILLS}/commits/")), 1, "commit selector resolved again")
        # Endpoint overrides without --test-gh never consult the developer's gh login.
        self.assertTrue(all(r.authorization is None for r in self.forge.requests))

    def test_github_source_extracts_a_folder_without_cloning(self):
        self.forge.commit(SKILLS, FILES, "main")
        acquired = self.acquire({"type": "github_source", "repo": SKILLS}, ref("main"), folder="skills")
        directory = Path(acquired["directory"])
        self.assertEqual((directory / "review" / "SKILL.md").read_bytes(), b"review skill")
        self.assertFalse((directory / "README.md").exists())
        self.assertFalse(any(self.sandbox.root.glob("source/*/repo")), "github_source created a clone")

    def test_git_raw_uses_platform_routes_and_token_file(self):
        self.forge.commit("group/sub/project", {"docs/guide.md": b"guide"}, "main", "v1")
        token = self.sandbox.dir / "platform.token"
        token.write_text("fixture-token\n", encoding="utf-8")
        for platform, selector in (("gitlab", ref("main")), ("gitea", {"type": "tag", "tag": "v1"})):
            acquired = self.acquire({"type": "git_raw", "host": f"{platform}.example.com", "platform": platform,
                                     "repo": "group/sub/project", "path": "docs/guide.md",
                                     "token_file": str(token)}, selector)
            self.assertEqual(Path(acquired["directory"], "guide.md").read_bytes(), b"guide")
        gitlab = self.forge.requested("/git/group/sub/project/-/raw/main/")
        gitea = self.forge.requested("/git/group/sub/project/raw/v1/")
        self.assertTrue(gitlab and all(r.private_token == "fixture-token" for r in gitlab))
        self.assertTrue(gitea and all(r.authorization == "token fixture-token" for r in gitea))


class BorrowedGhToken(Scenario):
    tags = frozenset({"platform-sources"})
    gh_token = "gho_fixture"

    def test_fixture_gh_token_is_sent_to_github(self):
        self.forge.commit(SKILLS, FILES, "main")
        uuid = self.client.register_source({"type": "github_raw", "repo": SKILLS, "path": "README.md"})["uuid"]
        self.client.acquire({"source_uuid": uuid, "selector": ref("main"), "selection": {},
                             "policy_refs": self.policy_refs()})
        github = self.forge.requested("/repos/") + self.forge.requested("/github-raw/")
        self.assertTrue(github)
        self.assertTrue(all(r.authorization == "Bearer gho_fixture" for r in github), github)
