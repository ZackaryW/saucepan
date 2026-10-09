## Purpose

Resolve provider inputs and platform-specific release assets while sharing cached source discovery independently of acquired output retention.

## ADDED Requirements

### Requirement: GitHub release sources acquire assets without cloning
A `github_release` source SHALL resolve latest stable releases or explicit release tags and expose their asset metadata for selection. It SHALL NOT clone or fetch a Git repository to acquire release assets. Resolved provenance SHALL identify the release and selected asset, not only a mutable latest-download URL.

#### Scenario: Saucepan acquires its own release
- **WHEN** its release source resolves a matching executable archive
- **THEN** acquisition downloads the selected asset and records its release/asset identity
- **AND** no Git checkout is created for that release source

### Requirement: Remote discovery uses a twenty-four-hour cache by default
Saucepan SHALL reuse successful discovery results for less than 86,400 seconds without contacting the source for another discovery check. The first use at or after expiry SHALL check the source. Cache keys SHALL distinguish release/ref selectors while allowing platform filters of the same selector to share discovery. Reads SHALL NOT extend freshness. Missing cache, forced refresh, or invalid future timestamps SHALL trigger a check.

#### Scenario: Multiple platform requests use fresh metadata
- **WHEN** Windows and Linux requests resolve the same release selector within the freshness interval
- **THEN** they select assets from the shared cached discovery result without separate discovery requests

#### Scenario: Expired discovery fails
- **WHEN** a due check fails
- **THEN** the last-success timestamp remains unchanged and the default request reports failure
- **AND** explicitly enabled stale fallback reports its stale status and actual resolved identity

### Requirement: Ordered filters select assets deterministically
Requests SHALL support platform OS/architecture constraints, ordered asset patterns, and a `selection_policy` targeting `github_release_asset_filter` with strategy `first_match_win`. The first applicable filter producing a nonempty match SHALL win. The initial single-asset contract SHALL fail for no matches or multiple assets in the winning filter. Binary-relative paths SHALL be validated against the resulting content.

#### Scenario: A platform-specific filter precedes a fallback
- **WHEN** both filters could produce an asset and the first filter matches the requested platform
- **THEN** the first filter determines selection independently of remote asset enumeration order

#### Scenario: The winning pattern is ambiguous
- **WHEN** the winning filter matches two assets
- **THEN** the request fails with an ambiguity diagnostic rather than arbitrarily downloading one

### Requirement: Repository sources maintain reusable source data
Git and `github_repo` sources SHALL maintain reusable repository data separately from outputs, resolve requested refs to exact commits, and acquire selected files from those commits. Future fetches SHALL NOT mutate published outputs. Required submodule and LFS contents SHALL be resolved consistently or acquisition SHALL fail without publishing incomplete content.

#### Scenario: A tracked branch advances
- **WHEN** a due refresh resolves a new commit and a selected folder is acquired
- **THEN** the new output records that exact commit and the previous output remains unchanged

### Requirement: URL and local sources use the same output contract
Saucepan SHALL support HTTP(S) file/ZIP and local file/directory inputs under the new acquisition contract. URL acquisitions SHALL record actual content identity; local acquisitions SHALL inspect the local input on use without applying a remote-discovery TTL. Raw download and supported extraction modes SHALL be explicit; unsupported formats SHALL fail clearly.

#### Scenario: Local content changes
- **WHEN** a local input is changed between acquisitions
- **THEN** the next acquisition observes its new content rather than reusing a 24-hour remote-discovery result
