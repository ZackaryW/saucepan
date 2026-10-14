from typing import Literal, NotRequired, TypedDict
from .source import Evidence, Platform, Selector

class Selection(TypedDict, total=False):
    platform: Platform
    filter_refs: list[str]
    format: Literal["natural", "raw", "zip"]
    filename: str
    folder: str
    bin: list[str]
    sha256: str

class AcquireRequest(TypedDict):
    source_uuid: str
    selector: Selector
    selection: NotRequired[Selection]
    policy_refs: NotRequired[list[str]]

class EnsureRequest(AcquireRequest):
    scope: str
    context: NotRequired[dict[str, str]]

class ApplicationRegistration(TypedDict):
    registration_key: str
    name: str
    policy_refs: NotRequired[list[str]]
    credentials_required: NotRequired[bool]

class ApplicationConfiguration(TypedDict):
    expected_revision: int
    policy_refs: list[str]
    credential: NotRequired[Literal["enable", "rotate", "disable"] | None]

class Application(TypedDict):
    uuid: str
    name: str
    revision: int
    state: str
    settings: Evidence
    active: Evidence | None
    shims: list[Evidence]

class Claim(TypedDict):
    uuid: str
    scope: str
    entry_uuid: str
    context: dict[str, str]
    policy_refs: list[str]
    request: Evidence
    effective: Evidence

class ClaimView(TypedDict):
    claim: Claim
    state: Literal["ready", "reclaim_pending", "reclaimed"]

class CoveredView(TypedDict):
    app: str
    claim: str
    entry: str
    relative: str | None

class OutputEntry(TypedDict):
    uuid: str
    state: Literal["ready", "reclaim_pending", "reclaimed"]
    request: Evidence
    resolved: Evidence
    content_digest: str
    sequence: int
    files: dict[str, Evidence]

class ReclaimReport(TypedDict):
    effective_keep: dict[str, int]
    reclaimed: list[str]
    deferred: dict[str, str]
    pending: dict[str, str]

class Acquired(TypedDict):
    entry: OutputEntry
    directory: str
    reused: bool
    update_checked: bool
    stale: bool
    reclaim: ReclaimReport

class Claimed(TypedDict):
    claim: Claim
    view: CoveredView
    acquired: Acquired

class ClaimAdoption(TypedDict):
    app: str
    revision: int
    claim: Claim

class Condition(TypedDict):
    key: str
    op: Literal["eq", "glob"]
    value: str

ClaimQuery = TypedDict("ClaimQuery", {"app": str, "scope": NotRequired[str], "where": list[Condition]})
DiscoveryQuery = TypedDict("DiscoveryQuery", {"scope": NotRequired[str], "where": list[Condition]})

class QueryResult(TypedDict):
    app: str
    revision: int
    claims: list[ClaimView]

class ReleaseResult(TypedDict):
    released: str
