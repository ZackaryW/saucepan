## MODIFIED Requirements

### Requirement: Full command-surface coverage
The SDK SHALL cover the complete CLI surface: install with an optional ref, update, uninstall, list, path, search, bucket add with an optional ref, bucket refresh, bucket remove, bucket list, and the index, buckets, sauce, and bucket `cat` targets. Search results SHALL be returned as raw parsed JSON values, because the output shape is determined by the caller's filter.

#### Scenario: Install accepts an optional ref
- **WHEN** a caller installs a target with a branch, tag, or commit
- **THEN** the SDK passes the ref to the install command and the resulting sauce records the requested ref and resolved commit

#### Scenario: Bucket add accepts an optional ref
- **WHEN** a caller registers a bucket with a branch, tag, or commit
- **THEN** the SDK passes the ref to `bucket add` and invalidates its cached bucket collection after success

#### Scenario: Bucket refresh is reachable
- **WHEN** a caller refreshes a registered bucket
- **THEN** the SDK invokes `bucket refresh`, invalidates cached bucket state, and returns normally on success

#### Scenario: Every cat target is reachable
- **WHEN** a caller requests the full index, the bucket list, a single sauce entry, or a bucket document
- **THEN** each is available through the SDK and returns the parsed JSON that target emits

#### Scenario: Search returns untyped parsed results
- **WHEN** a caller searches with a jq filter that projects arbitrary fields
- **THEN** the SDK returns the parsed JSON values produced by the filter without coercing them into entity objects

#### Scenario: Search surfaces a missing jq dependency
- **WHEN** a search is invoked in an environment where the configured jq is unavailable
- **THEN** the resulting failure is surfaced through the exception hierarchy rather than as an empty result

### Requirement: Python test governance scoped to the SDK
Python BDD and TDD stack configuration SHALL be scoped to the SDK directory so that it does not govern the Rust tree. The SDK's behavior scenarios and unit tests SHALL exercise an actually-built saucepan binary rather than substitutes for it, so that divergence between the SDK and the CLI is detected by the suite. Each SDK behavior capability SHALL own an independently runnable feature root with thin bindings and delegated shared lifecycle support.

#### Scenario: Rust tree is unaffected by Python stack configuration
- **WHEN** the governance stack configuration is resolved for the repository root
- **THEN** the Python stack applies to the SDK directory only, and Rust verification continues to run unchanged

#### Scenario: Suite drives the real binary
- **WHEN** the SDK's scenarios and tests execute
- **THEN** they invoke a built saucepan executable against a real workspace rather than a stubbed command layer

#### Scenario: Contract divergence fails the suite
- **WHEN** the CLI's flags, JSON output shape, or exit-code mapping change without a corresponding SDK change
- **THEN** the SDK suite fails rather than silently returning incorrect results

#### Scenario: SDK capability root runs independently
- **WHEN** an SDK behavior capability is selected for verification
- **THEN** its feature root runs independently from repository-root capabilities and other SDK capability roots

### Requirement: Sauce snapshots expose manifest source
A `Sauce` snapshot SHALL expose the source that supplied its manifest, so a caller can distinguish an entry described by its own repository from one described by a central index without parsing raw state.

#### Scenario: Reading manifest source from a snapshot
- **WHEN** a caller reads a sauce whose manifest came from a central index
- **THEN** the snapshot reports the index as its manifest source

#### Scenario: Repository-sourced snapshot
- **WHEN** a caller reads a sauce whose manifest came from its own repository
- **THEN** the snapshot reports the repository as its manifest source

### Requirement: Bucket stubs expose extra fields
Parsed bucket stubs SHALL expose fields beyond the required ones to callers, so consumer data carried in an index is reachable through the SDK rather than only through raw state reads.

#### Scenario: Stub carrying consumer data
- **WHEN** a caller reads stubs from a bucket whose entries carry extra fields
- **THEN** those fields are present on the returned stubs

#### Scenario: Stub without extra fields
- **WHEN** a caller reads stubs from a bucket whose entries carry only the required fields
- **THEN** the stubs expose an empty set of extra fields rather than failing
