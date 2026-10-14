from typing import Literal, NotRequired, TypedDict
from .claim import AcquireRequest
from .source import Evidence

class ExistingTarget(TypedDict):
    type: Literal["output", "claim"]
    uuid: str

class ClaimIntent(TypedDict):
    scope: str
    context: NotRequired[dict[str, str]]

class AcquireTarget(AcquireRequest):
    type: Literal["acquire"]
    claim: NotRequired[ClaimIntent]

class Member(TypedDict):
    name: str
    target: ExistingTarget | AcquireTarget
    relative: NotRequired[str]

class InlineMember(TypedDict):
    name: str
    target: ExistingTarget
    relative: NotRequired[str]

class GroupDefinition(TypedDict):
    registration_key: str
    members: list[Member]
    policy_refs: NotRequired[list[str]]

class GroupRequest(TypedDict):
    members: list[InlineMember]
    policy_refs: NotRequired[list[str]]

class SelectedMember(TypedDict):
    output_uuid: str
    claim_uuid: str | None
    scope: str | None
    relative: str | None
    selected_source: Evidence
    checked_source: Evidence | None
    last_success: int | None
    policy_provenance: list[Evidence]

class Generation(TypedDict):
    uuid: str
    members: dict[str, SelectedMember]

class GroupRecord(TypedDict):
    uuid: str
    owner: str | None
    revision: int
    definition: GroupDefinition
    witnesses: dict[str, Evidence]
    selected: Generation | None

class GroupMemberDescriptor(TypedDict):
    output_uuid: str
    claim_uuid: NotRequired[str]
    directory: str
    relative: str | None
    verification: Literal["metadata", "full"]
    policy_provenance: list[Evidence]

class GroupDescriptor(TypedDict):
    members: dict[str, GroupMemberDescriptor]

class RemoveResult(TypedDict):
    removed: str
