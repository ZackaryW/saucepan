import io
import json
import unittest
import urllib.error
import urllib.request
import zipfile

from acceptance.forge import Forge


def fetch(url, method="GET", headers=None):
    request = urllib.request.Request(url, method=method, headers=headers or {})
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            return response.status, response.read()
    except urllib.error.HTTPError as error:
        return error.code, error.read()


class ForgeRoutes(unittest.TestCase):
    def setUp(self):
        self.forge = Forge().start()
        self.addCleanup(self.forge.close)

    def test_release_discovery_and_asset_api(self):
        self.forge.release("Owner/Tool", "v1", {"tool.zip": b"one"})
        second = self.forge.release("owner/tool", "v2", {"tool.zip": b"two"})
        self.forge.release("owner/tool", "v3-rc", {"tool.zip": b"rc"}, prerelease=True)
        status, body = fetch(self.forge.github_api + "/repos/owner/tool/releases/latest")
        latest = json.loads(body)
        self.assertEqual((status, latest["tag_name"], latest["id"]), (200, "v2", second.id))
        asset = latest["assets"][0]
        self.assertTrue(asset["digest"].startswith("sha256:"))
        self.assertEqual(fetch(f"{self.forge.base}/repos/owner/tool/releases/assets/{asset['id']}"), (200, b"two"))
        self.assertEqual(fetch(asset["browser_download_url"])[0], 410)
        status, body = fetch(self.forge.github_api + "/repos/owner/tool/releases/tags/v1")
        self.assertEqual(json.loads(body)["tag_name"], "v1")

    def test_outage_applies_to_discovery_only(self):
        release = self.forge.release("o/r", "v1", {"a": b"x"})
        self.forge.fail = True
        self.assertEqual(fetch(self.forge.base + "/repos/o/r/releases/latest")[0], 503)
        asset_id = release.assets["a"][0]
        self.assertEqual(fetch(f"{self.forge.base}/repos/o/r/releases/assets/{asset_id}"), (200, b"x"))
        self.forge.fail = False
        self.assertEqual(fetch(self.forge.base + "/repos/o/r/releases/latest")[0], 200)

    def test_commits_raw_and_zipball(self):
        commit = self.forge.commit("o/r", {"skills/a/SKILL.md": b"skill", "README.md": b"readme"}, "main")
        self.assertEqual(fetch(self.forge.base + "/repos/o/r/commits/main"), (200, commit.encode()))
        self.assertEqual(fetch(f"{self.forge.base}/repos/o/r/commits/{commit}"), (200, commit.encode()))
        self.assertEqual(fetch(f"{self.forge.github_raw}/o/r/{commit}/skills/a/SKILL.md"), (200, b"skill"))
        status, body = fetch(f"{self.forge.base}/repos/o/r/zipball/{commit}")
        names = zipfile.ZipFile(io.BytesIO(body)).namelist()
        self.assertEqual(status, 200)
        self.assertEqual({n.split("/")[0] for n in names}, {f"o-r-{commit[:7]}"})
        self.assertIn(f"o-r-{commit[:7]}/skills/a/SKILL.md", names)

    def test_platform_raw_routes_and_request_log(self):
        self.forge.commit("group/sub/project", {"dir/file.txt": b"raw"}, "main", "v1")
        gitlab = self.forge.git_raw + "/group/sub/project/-/raw/main/dir/file.txt"
        gitea = self.forge.git_raw + "/group/sub/project/raw/v1/dir/file.txt"
        self.assertEqual(fetch(gitlab, headers={"PRIVATE-TOKEN": "glpat"}), (200, b"raw"))
        self.assertEqual(fetch(gitea, method="HEAD", headers={"Authorization": "token tea"}), (200, b""))
        log = self.forge.requested("/git/")
        self.assertEqual([(r.method, r.private_token, r.authorization) for r in log],
                         [("GET", "glpat", None), ("HEAD", None, "token tea")])

    def test_installer_downloads_and_missing_routes(self):
        self.forge.installer("v1", {"saucepan.zip": b"zip"})
        self.assertEqual(fetch(self.forge.releases_base + "/v1/saucepan.zip"), (200, b"zip"))
        self.assertEqual(fetch(self.forge.releases_base + "/v2/saucepan.zip")[0], 404)
        self.assertEqual(fetch(self.forge.base + "/repos/o/unknown/releases/latest")[0], 404)


if __name__ == "__main__":
    unittest.main()
