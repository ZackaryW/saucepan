"""Profile previews and lifecycle receipts; the manager owns policy evaluation."""
from typing import Literal, NotRequired, TypedDict
from .policy import Policy
from .source import Evidence
from .composition import Retrieval, Acquisition, SourceInput

type ProfileName = Literal['managed', 'explicit-update', 'temporary']

class ProfileExpansion(TypedDict):
    name: ProfileName
    for_install: bool
    digest: str
    catalog: list[Policy]
    retrieval_policy_refs: list[str]
    acquisition_policy_refs: list[str]
    combined_policy_refs: list[str]

class ExplainRetrieve(TypedDict):
    operation: Literal['retrieve']
    request: Retrieval
    scope: NotRequired[str | None]

class ExplainAcquire(TypedDict):
    operation: Literal['acquire']
    source: SourceInput
    request: Acquisition
    scope: NotRequired[str | None]

class ExplainCompose(TypedDict):
    operation: Literal['compose']
    retrieval: Retrieval
    acquisition: Acquisition
    scope: NotRequired[str | None]

class ExplainClaim(TypedDict):
    operation: Literal['claim']
    app: str
    claim: str

type ExplainRequest = ExplainRetrieve | ExplainAcquire | ExplainCompose | ExplainClaim
type Origin = Literal['built_in', 'source', 'application', 'request', 'adopted_claim']

class PolicyEvidence(TypedDict):
    uuid: str
    digest: str

class AxisExplanation(TypedDict):
    applicable: bool
    value: object
    origin: Origin | None
    evidence: PolicyEvidence | None

class RestrictionEvidence(TypedDict):
    origin: Origin
    record: Policy
    digest: str

class PolicyExplanation(TypedDict):
    axes: dict[str, AxisExplanation]
    restrictions: list[RestrictionEvidence]

class Probe(TypedDict):
    args: list[str]
    stdout_contains: str
    timeout_seconds: int

type Phase = Literal['preparing', 'commit_intent', 'cleanup_pending', 'complete', 'failed', 'conflict']

class Operation(TypedDict):
    operation: str
    app: str
    action: Evidence
    fingerprint: str
    status: Phase
    changed: bool
    expected_revision: int | None
    claim: str
    candidate: str | None
    desired: Evidence | None
    desired_registry: Evidence | None
    source_policy: Evidence | None
    shims: list[Evidence]
    pending: list[str]
    reason: Evidence | None
    leftovers: list[str]

class LifecycleResult[T](TypedDict):
    """Exit 2 is a recorded noncomplete outcome, not a transport failure."""
    exit_code: Literal[0, 2]
    result: T
