# source-retrieval Specification

## Purpose

Retrieve provider source material independently from acquired copies, with explicit caching, freshness, verification, path representation and ownership.

## Requirements

### Requirement: Retrieval returns source material before acquisition
Retrieval SHALL resolve and obtain original provider material and complete its selected cache behavior before returning. Its result SHALL identify the source, exact resolved provenance, material kind, source path, cache outcome, freshness and verification evidence. Git material SHALL be repository data at an identified commit; a downloaded ZIP SHALL remain an archive until acquisition requests extraction. Retrieval SHALL NOT create an acquired output, app claim or active binding. Git acquisition subpaths SHALL NOT fragment repository retrieval identity.

#### Scenario: One repository supplies two copied folders
- **WHEN** a caller retrieves a repository and acquires two different subpaths from its exact commit
- **THEN** the same retrieved repository can supply both copies without another provider fetch when no acquisition update is due
- **AND** the source path identifies repository data while each acquisition path identifies its copied selection

#### Scenario: An archive is retrieved without extraction
- **WHEN** a caller retrieves a release ZIP without requesting acquisition
- **THEN** the source path addresses the original ZIP and no extracted output or claim is created

### Requirement: Cache mode is an explicit retrieval policy
Retrieval SHALL support cached, temporary and no_store modes. Cached mode SHALL permit reusable discovery and source material. Temporary and no_store SHALL resolve and fetch fresh for each independent call without consulting or updating Saucepan discovery, repository-object, LFS, retrieval-material or acquired-output caches. Temporary SHALL own disposable scratch through a lifetime guard. No_store SHALL require an explicit caller-owned destination and SHALL NOT create managed identities or revision history. Local inputs SHALL be inspected and captured fresh rather than artificially fetched over a network. Scratch I/O SHALL be permitted; it SHALL NOT become a reusable cache. Inline fresh retrieval SHALL work without app/source registration, root initialization, home lookup or native key custody.

#### Scenario: Existing cache must not satisfy no-store retrieval
- **WHEN** matching source and output caches exist and the caller selects no_store
- **THEN** the provider is contacted again and the existing caches remain unread and unchanged
- **AND** provider failure fails the operation instead of returning the cached material

#### Scenario: Temporary retrieval without a home store
- **WHEN** a caller retrieves an inline source temporarily with no configured home or initialized store
- **THEN** it receives guard-owned fresh material and no Saucepan root or registration is created

### Requirement: Cached freshness is configurable independently of output history
Cached retrieval SHALL support max_age with a nonnegative interval, always and cache_only, with max_age 86400 seconds as its default. The effective policy SHALL be observable. Cache_only SHALL perform no provider network requests and fail if the required material or dependencies are unavailable. Explicit stale fallback SHALL apply only to cached retrieval and SHALL not advance a last-success timestamp after failed refresh. Trusted source-default configuration SHALL be available through native and CLI interfaces, serialized with provider refresh so neither overwrites concurrent changes. Per-call overrides SHALL not alter defaults. Incompatible policy combinations SHALL fail before effects.

#### Scenario: Override a source default for one call
- **WHEN** a caller requests always against a source whose default is max_age 86400
- **THEN** that retrieval checks the origin and later defaulted calls retain the original source policy

#### Scenario: Cache-only Git is missing LFS objects
- **WHEN** the repository commit is cached but required LFS material is missing under cache_only
- **THEN** retrieval reports unavailable material without fetching or returning an incomplete successful result

#### Scenario: No-store contradicts stale fallback
- **WHEN** no_store is combined with stale fallback or cache_only
- **THEN** validation fails before network requests, cache access or destination creation

### Requirement: Retrieved repositories coordinate access and complete dependencies
Git retrieval SHALL resolve a ref, tag or exact commit and obtain all required submodule and LFS material for subsequent offline export. Its source path SHALL identify repository representation explicitly, including whether it is bare, and SHALL not be presented as a checked-out acquired folder. Native retained access and leased process access SHALL coordinate Saucepan refresh and maintenance with repository readers. A held handle SHALL preserve its selected commit and required objects. Returned access SHALL not authorize caller mutation of shared repository configuration, refs or object storage. A copy/export from an adequate retrieved handle SHALL require no provider fetch.

#### Scenario: Repository refresh races a held source handle
- **WHEN** a source reader retains coordinated access while another caller requests refresh
- **THEN** the refresh waits within its deadline or reports contention rather than invalidating the reader's repository state
- **AND** already copied acquisitions remain unchanged

#### Scenario: Acquisition after disconnect
- **WHEN** complete repository material has been retrieved and network access becomes unavailable
- **THEN** acquisition with no due update can export a selected subpath including its recorded dependency content

### Requirement: Delivery and lifetime are explicit and failure-safe
Caller-owned source delivery SHALL publish only to a new, non-overlapping destination, preserve occupied destinations, and report failure without a successful partial path. Temporary material SHALL remain alive while native dependent handles or supported child-process lifetime handles retain it. CLI temporary retrieval SHALL require a lifetime-bearing consumer workflow and SHALL reject returning an already-expired bare path. Cached bare source lookup SHALL not refresh, acquire or retain a lease. Native/process guards SHALL protect against Saucepan operations, not arbitrary external writes. Cache cleanup SHALL defer live retrievals; explicit purge SHALL retire only the selected retrieval and SHALL not delete acquired copies or source registration.

#### Scenario: Temporary consumer outlives its wrapper
- **WHEN** a wrapper exits unexpectedly while a supported consumer retains its inherited lifetime handle
- **THEN** temporary source material remains available until that consumer's protected lifetime ends

#### Scenario: Cached source is purged after acquisition
- **WHEN** an unused cached retrieval is explicitly purged after an acquisition copy is published
- **THEN** that acquired copy remains usable and later retrieval can fetch the source again

### Requirement: Fresh retrieval preserves explicit application restrictions
An operation using an app identity SHALL verify its configured credentials and source/action/scope restrictions whether or not it creates a claim or uses cache. Missing required authorization scope SHALL fail without administrative fallback. Inline unregistered fresh operations SHALL be explicit local operations, not an alternative identity mode selectable by a denied app request. Registered fresh-mode reads SHALL not cause implicit store recovery or cache writes; a root needing recovery SHALL return a clear prerequisite error rather than violate the requested effects.

#### Scenario: An app tries no-store to bypass a forbidden source
- **WHEN** an app requests a source outside its allowance with no_store and no claim
- **THEN** access is denied before fetching or writing any result

#### Scenario: Fresh operation encounters pending store recovery
- **WHEN** a registered-source fresh retrieval would require replaying committed store recovery
- **THEN** it fails with guidance to reconcile separately and does not mutate the store implicitly
