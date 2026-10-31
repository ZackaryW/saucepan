"""Discovery freshness, expiry, provider outage and stale fallback, driven by the clock."""
from pathlib import Path

from saucepan_sdk import SaucepanError

from acceptance.scenario import Scenario, new_uuid, zipped

REPO = "fixture/fresh"


class Freshness(Scenario):
    tags = frozenset({"freshness"})

    def setUp(self):
        super().setUp()
        self.forge.release(REPO, "v1", {"tool.zip": zipped(**{"version.txt": b"one"})})
        self.source = self.client.register_source({"type": "github_release", "repo": REPO})["uuid"]
        self.filter = self.asset_filter("tool.zip")

    def discoveries(self) -> int:
        return len(self.forge.requested(f"/repos/{REPO}/releases/latest"))

    def version(self, acquired) -> bytes:
        return Path(acquired["directory"], "version.txt").read_bytes()

    def test_cache_reuse_then_expiry_triggers_discovery(self):
        request = self.release_request(self.source, self.policy_refs(), self.filter)
        first = self.client.acquire(request)
        self.assertEqual((self.version(first), self.discoveries()), (b"one", 1))
        self.forge.release(REPO, "v2", {"tool.zip": zipped(**{"version.txt": b"two"})})
        self.clock.advance(days=1, seconds=-1)
        cached = self.client.acquire(request)
        self.assertEqual((self.version(cached), self.discoveries()), (b"one", 1))
        self.clock.advance(seconds=1)
        expired = self.client.acquire(request)
        self.assertEqual((self.version(expired), self.discoveries()), (b"two", 2))
        self.assertTrue(all("/browser/" not in r.path for r in self.forge.requests))

    def test_outage_fails_unless_stale_results_are_allowed(self):
        strict = self.release_request(self.source, self.policy_refs(), self.filter)
        allow = {"uuid": new_uuid(), "type": "stale_policy", "allow_stale": True}
        lenient = self.release_request(self.source, self.policy_refs(retrieval=[allow]), self.filter)
        self.client.acquire(strict)
        self.client.acquire(lenient)
        self.forge.fail = True
        self.clock.advance(days=2)
        with self.assertRaises(SaucepanError):
            self.client.acquire(strict)
        stale = self.client.acquire(lenient)
        self.assertTrue(stale["stale"], stale)
        self.assertEqual(self.version(stale), b"one")
