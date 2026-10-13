## MODIFIED Requirements

### Requirement: Remote discovery uses a twenty-four-hour cache by default
For cached retrieval with the default refresh policy, Saucepan SHALL reuse successful discovery results for less than 86,400 seconds without contacting the source for another discovery check. Freshness and stale fallback SHALL be configured through independent policy references; source default adoption SHALL be revision checked and per-call selection SHALL NOT mutate the source defaults. The first retrieval requiring discovery at or after expiry SHALL check the source; expiry SHALL NOT imply a background timer. Cache keys SHALL distinguish release/ref selectors while allowing platform filters of the same selector to share discovery. Reads SHALL NOT extend freshness. Missing cache, forced refresh, or invalid future timestamps SHALL trigger a check during retrieval. A due acquisition update SHALL request fresh retrieval according to acquisition-refresh rather than silently accept the source TTL. Exact-output access, verification and rollback SHALL NOT refresh discovery. This cache SHALL govern discovery metadata, not eliminate downloads needed to obtain unavailable content. Temporary and no_store retrieval SHALL bypass this cache as specified by source-retrieval. Cached refresh modes and defaults SHALL be configurable independently of output revision policy.

#### Scenario: Multiple platform requests use fresh metadata
- **WHEN** Windows and Linux requests resolve the same release selector within the freshness interval
- **THEN** they select assets from the shared cached discovery result without separate discovery requests

#### Scenario: Expired discovery fails
- **WHEN** a due check fails
- **THEN** the last-success timestamp remains unchanged and the default request reports failure
- **AND** explicitly enabled stale fallback reports its stale status and actual resolved identity

### Requirement: Ordered filters select assets deterministically
Requests SHALL support platform OS/architecture constraints, an ordered list of UUID references to github_release_asset_filter records, and a referenced selection_policy targeting github_release_asset_filter with strategy first_match_win. Omitting the strategy reference SHALL retain the built-in first-match behavior. Filter definitions SHALL own platform constraints and asset patterns; requests SHALL NOT embed filter definitions or a selection-policy body. Patterns within a filter SHALL form alternative matches, not separate priority levels. The first applicable filter producing a nonempty match SHALL win. The single-asset contract SHALL fail for no matches or multiple assets in the winning filter. Unknown, duplicate or wrong-type filter references SHALL fail before provider work. Binary-relative paths SHALL be validated against the resulting content.

#### Scenario: A platform-specific filter precedes a fallback
- **WHEN** both filters could produce an asset and the first filter matches the requested platform
- **THEN** the first filter determines selection independently of remote asset enumeration order

#### Scenario: The winning pattern is ambiguous
- **WHEN** the winning filter matches two assets
- **THEN** the request fails with an ambiguity diagnostic rather than arbitrarily downloading one
