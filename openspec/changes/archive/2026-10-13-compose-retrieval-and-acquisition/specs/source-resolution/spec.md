## MODIFIED Requirements

### Requirement: Remote discovery uses a twenty-four-hour cache by default
For cached retrieval with the default refresh policy, Saucepan SHALL reuse successful discovery results for less than 86,400 seconds without contacting the source for another discovery check. The first retrieval requiring discovery at or after expiry SHALL check the source; expiry SHALL NOT imply a background timer. Cache keys SHALL distinguish release/ref selectors while allowing platform filters of the same selector to share discovery. Reads SHALL NOT extend freshness. Missing cache, forced refresh, or invalid future timestamps SHALL trigger a check during retrieval. A due acquisition update SHALL request fresh retrieval according to acquisition-refresh rather than silently accept the source TTL. Exact-output access, verification and rollback SHALL NOT refresh discovery. This cache SHALL govern discovery metadata, not eliminate downloads needed to obtain unavailable content. Temporary and no_store retrieval SHALL bypass this cache as specified by source-retrieval. Cached refresh modes and defaults SHALL be configurable independently of output revision policy.

#### Scenario: Multiple platform requests use fresh metadata
- **WHEN** Windows and Linux requests resolve the same release selector within the freshness interval
- **THEN** they select assets from the shared cached discovery result without separate discovery requests

#### Scenario: Expired discovery fails
- **WHEN** a due check fails
- **THEN** the last-success timestamp remains unchanged and the default request reports failure
- **AND** explicitly enabled stale fallback reports its stale status and actual resolved identity

### Requirement: Repository sources maintain reusable source data
Git and `github_repo` retrieval SHALL operate on repository data, resolve requested refs to exact commits and obtain required dependencies before returning. Cached retrieval SHALL maintain reusable repository data separately from outputs; temporary and no_store SHALL use fresh isolated repository material. Acquisition SHALL export/copy selected subpaths from the retrieved exact commit and SHALL not expose repository data as its copied output. Future fetches SHALL NOT mutate published outputs. Required submodule and LFS contents SHALL be retrieved consistently or retrieval SHALL fail without returning complete material. Copying from an adequate retrieval SHALL require no network; a due acquisition update SHALL explicitly request a new retrieval first.

#### Scenario: A tracked branch advances
- **WHEN** a due refresh resolves a new commit and a selected folder is acquired
- **THEN** the new output records that exact commit and the previous output remains unchanged

### Requirement: URL and local sources use the same output contract
Saucepan SHALL support HTTP(S) file/ZIP and local file/directory inputs under the current acquisition contract. URL retrieval SHALL report original downloaded material and its verification evidence; managed publication SHALL record actual content identity. Local retrieval SHALL inspect and capture input without applying a remote-discovery TTL. Ensuring an already available claim under `reuse` and the default manual update policy SHALL retain its selected output without implicitly preparing a new acquisition, as defined in [application-claims](../application-claims/spec.md). Raw retrieval and supported acquisition extraction modes SHALL be explicit; unsupported formats SHALL fail clearly.

#### Scenario: Local content changes
- **WHEN** a local input is changed between acquisitions
- **THEN** the next acquisition observes its new content rather than reusing a 24-hour remote-discovery result
