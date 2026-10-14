from typing import Literal, TypedDict
from .group import GroupDescriptor, GroupRecord
from .source import Evidence

class GroupPinRequest(TypedDict):
    registration_key: str
    policy_refs: list[str]

class PinRequest(GroupPinRequest):
    outputs: list[str]

class GroupPinReceipt(TypedDict):
    operation: Literal["prepare", "pin"]
    expected_revision: int
    group: GroupRecord
    stale: list[str]

class PinRecord(TypedDict):
    uuid: str
    owner: str | None
    request: PinRequest
    witnesses: list[Evidence]
    state: Literal["active", "released"]
    group: GroupPinReceipt | None

class PinOutput(TypedDict):
    path: str
    state: Literal["ready", "reclaim_pending", "reclaimed"]
    available: bool

class PinInspection(PinRecord):
    outputs: dict[str, PinOutput]

class PinnedOutputs(TypedDict):
    pin: PinRecord
    paths: dict[str, str]

class PreparedGroup(TypedDict):
    group: GroupRecord
    pin: PinRecord | None
    resources: GroupDescriptor
    stale: list[str]
