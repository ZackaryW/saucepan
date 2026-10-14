"""Explicit initial claim; subsequent lookups do not implicitly update it."""
from saucepan_sdk import Client
from saucepan_sdk.models.claim import EnsureRequest


def ensure_project(client: Client, app: str, source: str, scope: str) -> str:
    request: EnsureRequest = {
        "scope": scope,
        "source_uuid": source,
        "selector": {"type": "ref", "reference": "HEAD"},
        "selection": {"folder": "skills"},
    }
    result = client.for_app(app).ensure_claim(app, request)
    return result["claim"]["uuid"]


def project_selection(client: Client, app: str, scope: str) -> list[str]:
    result = client.for_app(app).query_claims({"app": app, "scope": scope, "where": []})
    return [view["claim"]["entry_uuid"] for view in result["claims"]]
