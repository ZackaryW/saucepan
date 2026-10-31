"""Base class for scenarios: each test method runs in its own sandbox."""
from __future__ import annotations

import atexit
import re
import unittest

from .sandbox import Run, Sandbox

_RUN: Run | None = None


def current_run() -> Run:
    """The runner installs a run; direct ``unittest`` use gets an isolated one lazily."""
    global _RUN
    if _RUN is None:
        _RUN = Run()
        _RUN.isolate()
        atexit.register(_RUN.restore)
    return _RUN


def install_run(run: Run) -> None:
    global _RUN
    _RUN = run


class Scenario(unittest.TestCase):
    tags: frozenset[str] = frozenset()
    gh_token: str | None = None

    def setUp(self) -> None:
        name = re.sub(r"[^A-Za-z0-9_.-]", "-", self.id().rsplit(".", 2)[-2] + "." + self._testMethodName)
        self.sandbox = Sandbox(current_run(), name, gh_token=self.gh_token)
        self.addCleanup(self.sandbox.close)
        self.forge = self.sandbox.forge
        self.client = self.sandbox.client
        self.admin = self.sandbox.admin
        self.clock = self.sandbox.clock

    @property
    def builds(self):
        return current_run().builds

    # -- shared request building ---------------------------------------------
    def policy_refs(self, profile: str = "managed", *, for_install: bool = False,
                    retrieval: list[dict] = (), acquisition: list[dict] = ()) -> list[str]:
        """Register a profile, optionally overriding leaves, and return its combined references."""
        custom = [*retrieval, *acquisition]
        expansion = self.client.profile(
            profile, for_install=for_install, catalog=custom or None,
            retrieval_policy_refs=[p["uuid"] for p in retrieval] or None,
            acquisition_policy_refs=[p["uuid"] for p in acquisition] or None)
        self.client.register_policies(expansion["catalog"])
        return expansion["combined_policy_refs"]

    def asset_filter(self, *patterns: str) -> str:
        policy = {"uuid": new_uuid(), "type": "github_release_asset_filter", "asset_patterns": list(patterns)}
        self.client.register_policies([policy])
        return policy["uuid"]

    def release_request(self, source: str, policy_refs: list[str], filter_ref: str, **selection) -> dict:
        return {"source_uuid": source, "selector": {"type": "latest_stable"},
                "selection": {"format": "zip", "filter_refs": [filter_ref], **selection},
                "policy_refs": policy_refs}


def new_uuid() -> str:
    import uuid
    return str(uuid.uuid4())


def zipped(**files: bytes) -> bytes:
    import io
    import zipfile
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as archive:
        for name, data in files.items():
            archive.writestr(name, data)
    return buffer.getvalue()
