from typing import Literal, NotRequired, TypedDict

type Json = None | bool | int | float | str | list[Json] | dict[str, Json]
type Evidence = dict[str, Json]

class LocalSource(TypedDict):
    type: Literal["local"]
    path: str

class UrlSource(TypedDict):
    type: Literal["url"]
    url: str

class GitRepoSource(TypedDict):
    type: Literal["git_repo"]
    origin: str
    token_file: NotRequired[str]

class GithubSource(TypedDict):
    type: Literal["github_repo", "github_release", "github_source"]
    repo: str

class GithubRawSource(TypedDict):
    type: Literal["github_raw"]
    repo: str
    path: str

class GitRawSource(TypedDict):
    type: Literal["git_raw"]
    host: str
    platform: Literal["gitlab", "gitea"]
    repo: str
    path: str
    token_file: NotRequired[str]

type Source = LocalSource | UrlSource | GitRepoSource | GithubSource | GithubRawSource | GitRawSource

class SimpleSelector(TypedDict):
    type: Literal["local", "url", "latest_stable"]

class TagSelector(TypedDict):
    type: Literal["tag"]
    tag: str

class RefSelector(TypedDict):
    type: Literal["ref"]
    reference: str

class CommitSelector(TypedDict):
    type: Literal["commit"]
    commit: str

type Selector = SimpleSelector | TagSelector | RefSelector | CommitSelector

class Platform(TypedDict):
    os: str
    arch: str

class SourceQuery(TypedDict, total=False):
    uuid: str
    type: Literal["local", "url", "git_repo", "github_repo", "github_release", "github_raw", "git_raw", "github_source"]
    repo: str
    origin: str
    url: str

class SourceView(TypedDict):
    uuid: str
    source: Source
    public_discovery: bool

class SourceQueryResult(TypedDict):
    generation: int
    sources: list[SourceView]

class SourceRecord(TypedDict):
    uuid: str
    source: Source
    policy_revision: int
    policy_refs: list[str]
    discovery: dict[str, Evidence]

class InitResult(TypedDict):
    root: str
    integrity: Literal["basic", "keyring"]

class Record(TypedDict):
    type: Literal["application", "output", "retrieval", "acquisition"]
    uuid: str
    # inspect preserves all Rust fields; use operation-specific records below
    # for domain access rather than treating historical provenance as input.
    revision: NotRequired[int]
