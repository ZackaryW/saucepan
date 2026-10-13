## MODIFIED Requirements

### Requirement: CLI operations default to the user-home store
When `--root` is omitted, store-backed CLI operations SHALL resolve the operating-system user's home directory and append `.saucepan`. On Windows the default SHALL use USERPROFILE and on Unix HOME, with the standard operating-system home lookup fallback. An explicit `--root PATH` SHALL take precedence without requiring home resolution; relative overrides SHALL remain relative to the working directory. The CLI SHALL NOT search working-directory ancestors or fall back to another store after an error. Unavailable, empty or relative home resolution SHALL produce `invalid_request` with guidance to pass `--root`. Native APIs and shim-carried explicit roots SHALL retain their existing contracts. Inline temporary/no_store retrieval and its caller-owned or temporary acquisition SHALL not require home lookup or initialize a store.

#### Scenario: Default location is independent of working directory
- **WHEN** a caller omits --root while working in a project with a local .saucepan folder
- **THEN** the operation uses the user-home .saucepan store
- **AND** the project-local store is not read or modified

#### Scenario: Override remains isolated
- **WHEN** an explicit root is supplied and the default home store exists or home lookup is unavailable
- **THEN** only the explicit root is used

### Requirement: Read commands do not implicitly initialize a missing store
`saucepan init` SHALL initialize the resolved default or explicit root in basic mode unless `--integrity keyring` is explicitly selected. `--integrity basic` SHALL explicitly select the default. Native creation SHALL provide the same default and explicit choice. Successful initialization SHALL identify the selected mode. Invalid mode values SHALL fail before initialization effects. Other store-backed commands encountering a missing root directory SHALL fail with `unsupported_store`, no success stdout, and guidance to initialize the selected root; they SHALL NOT create it or its signing key. Existing corrupt, legacy or incomplete stores SHALL retain existing error handling without an implicit reset. Help and version SHALL succeed without home lookup or store initialization. Explicit inline fresh operations SHALL remain usable without any store and SHALL not create one as a side effect.

#### Scenario: First query precedes initialization
- **WHEN** a caller runs saucepan query-sources before the default root exists
- **THEN** the command fails with guidance to run saucepan init
- **AND** the root remains absent

#### Scenario: Initialization followed by querying
- **WHEN** a caller initializes a disposable user-home store and then queries without --root
- **THEN** both commands use the same .saucepan directory

#### Scenario: Default initialization has no native custody prerequisite
- **WHEN** a caller runs `saucepan init` and `saucepan query-sources` without a configured keyring
- **THEN** initialization reports basic mode and the query succeeds subject to existing source visibility
- **AND** neither command provisions or reads a signing key

## ADDED Requirements

### Requirement: Cached retrievals and acquisition check state have explicit record ownership
Cached retrieval material SHALL live under the owning source's working-data area. Persistent retrieval descriptors and managed acquisition check state SHALL have distinct typed records under index/<uuid>/index.json and participate in root membership, selected basic/keyring integrity and recoverable publication. Git working repositories SHALL remain provider-owned data, separately identified from acquired store/<entry-uuid>/ copies. Output revision reclaim SHALL not delete retrieval records or source material. Explicit retrieval purge SHALL coordinate source lifetimes without deleting independent acquired results or source registrations. Fresh-mode scratch and caller-owned destinations SHALL remain outside the persistent index and catalog.

#### Scenario: Output history is removed while source remains cached
- **WHEN** revision policy reclaims an old acquired output
- **THEN** its provenance remains and the source's retrieval cache remains independently available

#### Scenario: No-store operation completes
- **WHEN** an inline no_store retrieval and caller-owned acquisition succeed
- **THEN** the caller receives completed files without source, index, store, claim or catalog records being added to a Saucepan root
