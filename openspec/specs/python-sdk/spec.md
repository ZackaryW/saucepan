## Purpose

Define the vendorable, standard-library-only Python SDK that drives the saucepan CLI through a workspace-rooted object model while preserving the CLI's command, JSON, path, and exit-code contracts.

## Requirements

### Requirement: Workspace-rooted object model
The Python SDK SHALL expose `Workspace` as its entry point, constructed from a saucepan root path, and SHALL expose installed sauces and registered buckets as collections of `Sauce` and `Bucket` objects rather than as raw command output. A `Sauce` SHALL carry the manifest identity and metadata the index provides and SHALL expose its own lifecycle operations; a `Bucket` SHALL carry its URL and expose its own removal and stub-listing operations.

#### Scenario: Workspace yields sauce entities
- **WHEN** a caller reads the sauce collection of a workspace containing installed sauces
- **THEN** it receives `Sauce` objects exposing name, version, and description rather than parsed dictionaries or raw text

#### Scenario: Optional revision metadata is carried
- **WHEN** a `Sauce` is built from an index entry recording a requested ref and resolved commit
- **THEN** those values are readable on the object, and a `Sauce` built from an entry lacking them exposes their absence without error

#### Scenario: Lifecycle operations live on the entity
- **WHEN** a caller updates or uninstalls an installed sauce
- **THEN** the operation is invoked on that `Sauce` object and drives the corresponding CLI command

#### Scenario: Install returns the resulting entity
- **WHEN** a caller installs a target through the workspace
- **THEN** it receives the `Sauce` for the newly installed manifest name

#### Scenario: Buckets are entities
- **WHEN** a caller reads the bucket collection of a workspace with registered buckets
- **THEN** it receives `Bucket` objects that can list their stubs and remove themselves

### Requirement: Cached index with mutation invalidation
`Workspace` SHALL read and parse the index once and SHALL serve subsequent reads from that cached state. Every mutating operation SHALL invalidate the cache. Updating a sauce SHALL additionally refresh that entry so the object reports post-update metadata without an explicit refresh. `Workspace` SHALL expose an explicit refresh that discards cached state and re-reads.

#### Scenario: Repeated reads do not re-invoke the CLI
- **WHEN** a caller reads the sauce collection twice with no intervening mutation
- **THEN** the index is read from the binary once and the second read is served from cache

#### Scenario: Mutation invalidates cached state
- **WHEN** a caller installs or uninstalls a sauce after having read the sauce collection
- **THEN** the next read reflects the mutation rather than the previously cached collection

#### Scenario: Update refreshes its own entity
- **WHEN** a caller updates a sauce whose upstream manifest version has advanced
- **THEN** reading the version from that same `Sauce` object reports the new version without an explicit refresh

#### Scenario: Explicit refresh re-reads
- **WHEN** the index changes outside the SDK and the caller invokes the workspace refresh
- **THEN** subsequent reads reflect the externally changed index

#### Scenario: Bucket mutation invalidates cached state
- **WHEN** a caller adds or removes a bucket after having read the bucket collection
- **THEN** the next read reflects the mutation

### Requirement: Explicit binary resolution
`Workspace` SHALL accept a binary argument identifying the saucepan executable and SHALL default it to the name `saucepan` resolved through `PATH`. The SDK SHALL NOT download, build, or otherwise acquire a binary.

#### Scenario: Default resolves through PATH
- **WHEN** a workspace is constructed without a binary argument
- **THEN** commands are invoked using the name `saucepan` as resolved by the operating system's executable search

#### Scenario: Explicit binary is used verbatim
- **WHEN** a workspace is constructed with an explicit path to a saucepan executable
- **THEN** that exact executable is invoked and no search is performed

#### Scenario: Missing binary reports clearly
- **WHEN** a workspace is constructed with a binary that cannot be found or executed
- **THEN** the failure names the binary that could not be run rather than surfacing an unhandled operating-system error

### Requirement: Exit codes map to a typed exception hierarchy
The SDK SHALL raise exceptions derived from a common base for non-zero exit codes, with a distinct type for each documented category: not found (1), source error (2), config error (3), conflict (4), and internal error (5). Every raised exception SHALL carry the originating exit code and the captured standard-error output. A zero exit SHALL NOT raise.

#### Scenario: Missing sauce raises not-found
- **WHEN** a caller requests the path of a sauce that is not installed
- **THEN** the SDK raises the not-found exception type carrying exit code 1

#### Scenario: Name conflict raises conflict
- **WHEN** an install reuses an installed manifest name from a different source type or origin
- **THEN** the SDK raises the conflict exception type carrying exit code 4

#### Scenario: Invalid workspace raises config error
- **WHEN** a workspace is rooted at a directory containing no valid `saucepan.toml`
- **THEN** the SDK raises the config-error exception type carrying exit code 3

#### Scenario: Unreachable source raises source error
- **WHEN** an install fails because the source cannot be fetched
- **THEN** the SDK raises the source-error exception type carrying exit code 2

#### Scenario: Every raised exception carries diagnostics
- **WHEN** any non-zero exit is surfaced as an exception
- **THEN** the exception exposes the exit code and the command's captured standard-error text, and is catchable through the common base type

#### Scenario: Success does not raise
- **WHEN** a command exits zero
- **THEN** no exception is raised and the parsed result is returned

### Requirement: Full command-surface coverage
The SDK SHALL cover the complete CLI surface: install with an optional ref, update, uninstall, list, path, search, bucket add, bucket remove, bucket list, and the index, buckets, sauce, and bucket `cat` targets. Search results SHALL be returned as raw parsed JSON values, because the output shape is determined by the caller's filter.

#### Scenario: Install accepts an optional ref
- **WHEN** a caller installs a target with a branch, tag, or commit
- **THEN** the SDK passes the ref to the install command and the resulting sauce records the requested ref and resolved commit

#### Scenario: Every cat target is reachable
- **WHEN** a caller requests the full index, the bucket list, a single sauce entry, or a bucket document
- **THEN** each is available through the SDK and returns the parsed JSON that target emits

#### Scenario: Search returns untyped parsed results
- **WHEN** a caller searches with a jq filter that projects arbitrary fields
- **THEN** the SDK returns the parsed JSON values produced by the filter without coercing them into entity objects

#### Scenario: Search surfaces a missing jq dependency
- **WHEN** a search is invoked in an environment where the configured jq is unavailable
- **THEN** the resulting failure is surfaced through the exception hierarchy rather than as an empty result

### Requirement: Delegated artifact-path resolution
The SDK SHALL obtain a sauce's on-disk location by invoking the `path` command and SHALL NOT reconstruct the workspace directory layout itself.

#### Scenario: Path comes from the binary
- **WHEN** a caller reads the on-disk path of an installed sauce whose repository target contains a slash
- **THEN** the value is the path reported by the `path` command, and the SDK contains no independent encoding of the repository-directory rule

### Requirement: Independently vendorable, standard-library-only module
The SDK package SHALL import only the Python standard library at runtime. Test and development tooling SHALL be declared as development dependencies only. The SDK SHALL NOT import the Python platform resolver, so that either directory can be consumed on its own.

#### Scenario: Runtime imports are standard library only
- **WHEN** the SDK package and all of its modules are imported
- **THEN** every import resolves within the Python standard library

#### Scenario: Development tooling stays out of the runtime
- **WHEN** the project's dependency declarations are inspected
- **THEN** the test and BDD tooling appears only as development dependencies and the distribution declares no runtime dependencies

#### Scenario: SDK works without the resolver present
- **WHEN** only the SDK directory is materialized in a consumer's working tree and the resolver directory is absent
- **THEN** the package imports and constructs a workspace successfully

#### Scenario: Resolver works without the SDK present
- **WHEN** only the resolver directory is materialized and the SDK directory is absent
- **THEN** the resolver continues to function as specified

### Requirement: Excluded from the published Rust crate
The `sdk/` directory SHALL be excluded from the published `saucepan` Rust crate package.

#### Scenario: Crate package omits the SDK
- **WHEN** the `saucepan` crate is packaged for publishing
- **THEN** `sdk/` is not included in the package file list

### Requirement: Python test governance scoped to the SDK
Python BDD and TDD stack configuration SHALL be scoped to the SDK directory so that it does not govern the Rust tree. The SDK's behavior scenarios and unit tests SHALL exercise an actually-built saucepan binary rather than substitutes for it, so that divergence between the SDK and the CLI is detected by the suite.

#### Scenario: Rust tree is unaffected by Python stack configuration
- **WHEN** the governance stack configuration is resolved for the repository root
- **THEN** the Python stack applies to the SDK directory only, and Rust verification continues to run unchanged

#### Scenario: Suite drives the real binary
- **WHEN** the SDK's scenarios and tests execute
- **THEN** they invoke a built saucepan executable against a real workspace rather than a stubbed command layer

#### Scenario: Contract divergence fails the suite
- **WHEN** the CLI's flags, JSON output shape, or exit-code mapping change without a corresponding SDK change
- **THEN** the SDK suite fails rather than silently returning incorrect results

#### Scenario: SDK BDD is separate from repository-root Gherkin
- **WHEN** the SDK's behavior scenarios are executed
- **THEN** they run from the SDK project's own feature directory and step definitions, leaving the repository-root feature files untouched

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

### Requirement: Additive SDK compatibility
The additions in this change SHALL be additive. Existing SDK call signatures, return types, cache invalidation behavior, and the documented exit-code to exception mapping SHALL remain unchanged.

#### Scenario: Existing caller is unaffected
- **WHEN** an existing caller written against the prior SDK runs against this version
- **THEN** every existing call behaves identically and no exception mapping changes

### Requirement: Runtime dependency boundary is preserved
The SDK SHALL remain standard-library-only and independently vendorable after these additions, importing neither the platform resolver nor any third-party package.

#### Scenario: Import check after the change
- **WHEN** the SDK modules are walked for absolute top-level imports
- **THEN** every one resolves to the standard library, and all other imports are package-relative
