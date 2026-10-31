"""Revision retention across independent app claims, and identical-content reuse."""
from acceptance.scenario import Scenario, new_uuid, zipped

REPO = "fixture/retained"


class Retention(Scenario):
    tags = frozenset({"retention"})

    def setUp(self):
        super().setUp()
        self.forge.release(REPO, "v1", {"tool.zip": zipped(**{"data.txt": b"same bytes"})})
        self.source = self.client.register_source({"type": "github_release", "repo": REPO})["uuid"]
        self.filter = self.asset_filter("tool.zip")

    def claims(self, app):
        return {view["claim"]["uuid"]: view for view in self.client.claims(app)}

    def test_eleven_revisions_reclaim_the_oldest_and_claims_move_on(self):
        new_entry = {"uuid": new_uuid(), "type": "identical_pull_policy", "strategy": "new_entry"}
        request = self.release_request(self.source, self.policy_refs(acquisition=[new_entry]), self.filter)
        app_a = self.client.register_application({"registration_key": "consumer-a", "name": "Consumer A"})["uuid"]
        app_b = self.client.register_application({"registration_key": "consumer-b", "name": "Consumer B"})["uuid"]
        first = self.client.acquire(request, app=app_a, scope="acceptance/retention")
        other = self.client.acquire(request, app=app_b, scope="acceptance/retention")
        oldest = first["claim"]["entry_uuid"]
        for _ in range(9):
            self.client.acquire(request)
        # Ten latest revisions are kept; the first of eleven is reclaimed even though a claim names it.
        self.assertEqual(self.client.inspect(oldest)["state"], "reclaimed")
        self.assertEqual(self.claims(app_a)[first["claim"]["uuid"]]["claim"]["entry_uuid"], oldest)
        refreshed = self.client.update_claim(app_a, first["claim"]["uuid"])
        self.assertNotEqual(refreshed["claim"]["entry_uuid"], oldest)
        self.assertEqual(self.claims(app_b)[other["claim"]["uuid"]]["claim"]["entry_uuid"],
                         other["claim"]["entry_uuid"])

    def test_identical_content_is_reused_by_default(self):
        request = self.release_request(self.source, self.policy_refs(), self.filter)
        first = self.client.acquire(request)
        again = self.client.acquire(request)
        self.assertFalse(first["reused"])
        self.assertTrue(again["reused"])
        self.assertEqual(again["entry"]["uuid"], first["entry"]["uuid"])
