from typing import Literal, NotRequired, TypedDict
from .source import Evidence, Platform, Selector, Source

class InlineSource(TypedDict):
    kind: Literal["inline"]
    source: Source

class RegisteredSource(TypedDict):
    kind: Literal["registered"]
    source_uuid: str

type SourceInput = InlineSource | RegisteredSource

class AssetSelection(TypedDict, total=False):
    platform: Platform
    filter_refs: list[str]

class Retrieval(TypedDict):
    source: SourceInput
    selector: Selector
    asset: NotRequired[AssetSelection]
    policy_refs: NotRequired[list[str]]
    destination: NotRequired[str]
    expected_sha256: NotRequired[str]

class SimpleDestination(TypedDict):
    mode: Literal["managed", "temporary"]

class DirectoryDestination(TypedDict):
    mode: Literal["directory"]
    path: str

class Acquisition(TypedDict, total=False):
    destination: SimpleDestination | DirectoryDestination
    format: Literal["natural", "raw", "zip"]
    filename: str
    folder: str
    bin: list[str]
    policy_refs: list[str]

class CompositionRequest(TypedDict):
    retrieval: Retrieval
    acquisition: Acquisition

class ManagedMaterial(TypedDict):
    mode: Literal["managed"]
    uuid: str

class Ownership(TypedDict):
    kind: Literal["managed", "temporary", "caller"]
    uuid: NotRequired[str]

class RetrievedDescriptor(TypedDict):
    ownership: Ownership
    path: str
    kind: Evidence
    source: SourceInput
    resolved: Evidence
    dependencies: dict[str, str]
    verification: Evidence
    checked_at: int | None
    origin_checked: bool
    reused: bool
    stale: bool

class RetrievalReceipt(TypedDict):
    request: Retrieval
    descriptor: RetrievedDescriptor

class CallerMaterial(RetrievalReceipt):
    mode: Literal["caller"]

class AcquireRetrievedRequest(TypedDict):
    retrieval: ManagedMaterial | CallerMaterial
    acquisition: Acquisition

class AcquiredDescriptor(TypedDict):
    ownership: Ownership
    path: str
    selected_source: Evidence
    checked_source: Evidence | None
    verification: Evidence
    reused: bool
    changed: bool
    stale: bool
    update_policy: Evidence
    revision_policy: Evidence | None
    reclaim: Evidence | None

class ResolvedResource(TypedDict):
    entry_uuid: str
    directory: str
    verification: Literal["metadata", "full"]

class SourcePath(TypedDict):
    path: str

class PurgeResult(TypedDict):
    purged: bool
