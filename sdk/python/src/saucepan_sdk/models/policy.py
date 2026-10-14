from typing import Literal, NotRequired, TypedDict
from .source import Platform

type Action = Literal["read", "discover", "acquire", "update", "release", "recover", "lease", "pin", "unpin", "activate", "remove", "set_context", "register_policy"]

class Base(TypedDict):
    uuid: str

class AccessPolicy(Base):
    type: Literal["access_policy"]
    sources: NotRequired[list[str]]
    scopes: NotRequired[list[str]]
    actions: NotRequired[list[Action]]

class SourceVisibilityPolicy(Base):
    type: Literal["source_visibility_policy"]
    public_discovery: bool

class ClaimVisibilityPolicy(Base):
    type: Literal["claim_visibility_policy"]
    public_scopes: list[str]

class CachePolicy(Base):
    type: Literal["cache_policy"]
    mode: Literal["cached", "temporary", "no_store"]

class ImmediateRefreshPolicy(Base):
    type: Literal["refresh_policy"]
    mode: Literal["always", "cache_only"]

class TimedPolicy(Base):
    type: Literal["refresh_policy", "update_policy"]
    mode: Literal["max_age"]
    seconds: int

class UpdatePolicy(Base):
    type: Literal["update_policy"]
    mode: Literal["manual", "always"]

class StalePolicy(Base):
    type: Literal["stale_policy"]
    allow_stale: bool

class PayloadVerificationPolicy(Base):
    type: Literal["verification_policy"]
    policy_target: Literal["payload"]
    mode: Literal["none", "if_available", "required"]

class PathVerificationPolicy(Base):
    type: Literal["verification_policy"]
    policy_target: Literal["path"]
    mode: Literal["metadata", "full"]

class AssetFilterPolicy(Base):
    type: Literal["github_release_asset_filter"]
    platform: NotRequired[Platform | None]
    asset_patterns: list[str]

class SelectionPolicy(Base):
    type: Literal["selection_policy"]
    policy_target: Literal["github_release_asset_filter"]
    strategy: Literal["first_match_win"]

class IdenticalPullPolicy(Base):
    type: Literal["identical_pull_policy"]
    strategy: Literal["reuse", "new_entry"]

class NoHistoryPolicy(Base):
    type: Literal["revision_policy"]
    history: Literal["none"]

class LatestHistoryPolicy(Base):
    type: Literal["revision_policy"]
    history: Literal["latest"]
    keep: int

class ReclaimPolicy(Base):
    type: Literal["reclaim_policy"]
    strategy: Literal["report_unavailable", "restore_exact", "refresh"]

class UseableAppPolicy(Base):
    type: Literal["useable_app_policy"]

class PinPolicy(Base):
    type: Literal["pin_policy"]
    retention: Literal["until_released"]

class PolicyConstraints(Base):
    type: Literal["policy_constraints"]
    minimum_keep: NotRequired[int]
    maximum_keep: NotRequired[int]
    allowed_identical: NotRequired[list[Literal["reuse", "new_entry"]]]
    allowed_reclaim: NotRequired[list[Literal["report_unavailable", "restore_exact", "refresh"]]]
    allow_useable_app: NotRequired[bool]
    require_useable_app: NotRequired[bool]
    allow_automatic_updates: NotRequired[bool]
    minimum_update_interval: NotRequired[int]

type Policy = (AccessPolicy | SourceVisibilityPolicy | ClaimVisibilityPolicy | CachePolicy
               | ImmediateRefreshPolicy | TimedPolicy | UpdatePolicy | StalePolicy
               | PayloadVerificationPolicy | PathVerificationPolicy | AssetFilterPolicy
               | SelectionPolicy | IdenticalPullPolicy | NoHistoryPolicy | LatestHistoryPolicy
               | ReclaimPolicy | UseableAppPolicy | PinPolicy | PolicyConstraints)

class ApplicationAdoption(TypedDict):
    app: str
    revision: int
    policy_refs: list[str]
