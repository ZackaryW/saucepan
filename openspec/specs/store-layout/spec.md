# store-layout Specification

## Purpose

Separate source working data, acquisition metadata, and stored outputs so their lifecycles remain independent and applications can resolve content by UUID.

## Requirements

### Requirement: Source metadata and output content have separate locations
Saucepan SHALL keep source definitions under source/<source-uuid>.json, optional provider working data under source/<source-uuid>/, live application and output metadata under index/<uuid>/index.json, application claim registries under claims/<app-uuid>/index.json, separate claim envelopes under claims/<app-uuid>/<claim-uuid>.json, standalone policy definitions under policies/<policy-uuid>.json, and available acquired content under store/<entry-uuid>/. Persistent retrieval and acquisition-check records SHALL retain their distinct index record types. Applications, sources and claims SHALL retain policy references rather than embedded authoritative policy definitions or defaults/override bundles. Policy records SHALL participate in verified catalog membership and recoverable publication. Metadata SHALL remain readable and verified using the store basic checksums or keyring-backed HMAC tags. App and output records SHALL have distinguishable types. Completed uninstall SHALL retire the live app record while retaining its operation receipt; output reclaim SHALL retain provenance. Policy definitions referenced by historical records SHALL not be deleted by uninstall or reclaim. UUID and path validation SHALL prevent access outside the owning root.

#### Scenario: Application and output records coexist
- **WHEN** an application acquires content from a registered source
- **THEN** its app index identifies its claim registry, whose claim references an output index with an entry UUID resolving to the corresponding store directory
- **AND** the application does not require a store directory of its own

### Requirement: Stored contents preserve the acquisition structure
Saucepan SHALL place selected files, Git exports, raw downloads, or extracted contents at the output root without imposing an `items/` directory. Package-owned files named `index.json` SHALL remain ordinary content, and SHALL NOT define Saucepan metadata or policy scope.

#### Scenario: An archive contains its own index document
- **WHEN** a ZIP containing `index.json` and `assets/icon.png` is acquired in extraction mode
- **THEN** both files retain their relative locations under the output root without being interpreted as Saucepan configuration

### Requirement: Publication never exposes incomplete content as ready
Saucepan SHALL publish an output as ready only after acquisition and required validation complete. Interrupted or concurrent publication SHALL recover to consistent output metadata and app claims. Failed operations SHALL NOT redirect an existing claim or active binding to incomplete content.

#### Scenario: Process stops between content preparation and claim publication
- **WHEN** acquisition is interrupted after staging content but before committing its claim
- **THEN** recovery preserves the previous claim or completes a fully prepared committed transition
- **AND** no path lookup reports a partial output as ready

### Requirement: Legacy data is not implicitly accepted or migrated
Saucepan SHALL identify the flat-policy layout through a distinct named store contract without inheriting the old FORMAT_VERSION or compatibility decoders. Opening a legacy store, earlier inline-claim layout or preceding nested-policy composition store SHALL fail explicitly with unsupported_store before recovery, signing-key provisioning or data mutation. New flat stores SHALL have an explicit store UUID and contract discriminator. Missing or corrupt required claim or policy metadata SHALL NOT be treated as an empty new store. Known flat stores containing mixed inline and referenced policy authorities SHALL fail validation without fallback. No automatic conversion or in-place migration is provided.

#### Scenario: An old encrypted central index is present
- **WHEN** initialization or opening detects a legacy Saucepan store
- **THEN** it reports that a separate new-format root is required and leaves the legacy store unchanged

#### Scenario: Previous composition store is opened
- **WHEN** a root uses the preceding nested-policy contract
- **THEN** the binary reports unsupported_store with guidance to use a separate fresh root
- **AND** the source data, metadata, pending journal and credentials remain unchanged

### Requirement: An explicit root isolates the full application flow
Saucepan SHALL accept an explicit root for initialization and subsequent operations, and SHALL preserve that root through its managed Saucepan shim and child Saucepan execution. Explicit-root operations SHALL NOT silently read or mutate the production root. A fresh simulated root SHALL be usable directly without an additional nested `.saucepan` directory. The repository's existing `mock/` is historical evidence under an earlier contract; current acceptance SHALL preserve it and use a separate fresh root rather than treating explicit root selection as migration permission.

#### Scenario: The simulated installation launches its managed executable
- **WHEN** the caller invokes the shim from a fresh simulated root's `bin/` directory with that explicit root
- **THEN** source, index, store, claim, activation, and reclaim operations resolve within that root
- **AND** the user's production installation is not used as a fallback

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

### Requirement: Store integrity mode is persistent and unambiguous
Every newly initialized store SHALL have a store identity and an unambiguous integrity contract selecting basic or keyring mode. Subsequent opening, queries, mutations, recovery and shim execution SHALL use that persisted mode without repeated flags. Interrupted initialization SHALL reserve both identity and mode; a retry with a conflicting mode SHALL fail without converting state. The exact supported current-format protected store and its initialization reservation SHALL remain protected when they lack a new explicit mode field. Missing or malformed metadata SHALL NOT be interpreted as a basic store. Earlier unsigned, encrypted and inline-claim stores SHALL remain unsupported. Unknown integrity modes/contracts SHALL produce `unsupported_store`; corrupted recognized envelopes and mixed-mode members SHALL fail integrity validation. Opening SHALL NOT rewrite metadata just to add a mode. Existing live stores SHALL NOT support in-place mode conversion through initialization or ordinary operations.

#### Scenario: Existing protected store cannot access its key
- **WHEN** a supported protected store opens while its keyring is unavailable
- **THEN** it returns `key_unavailable` without replacing keys, rewriting envelopes or trying basic verification

#### Scenario: Initialization is interrupted
- **WHEN** initialization reserves a keyring-mode store and is interrupted before publishing its root
- **THEN** a keyring-mode retry can resume with the same identity
- **AND** a basic-mode retry rejects the reservation without changing its mode

#### Scenario: Unknown contract and older unsigned data
- **WHEN** a root has an unknown mode or contains an unsupported earlier unsigned layout
- **THEN** opening fails with `unsupported_store` and leaves its data unchanged

#### Scenario: Store is reopened by another process
- **WHEN** the manager or shim opens a valid basic store without an integrity option
- **THEN** it selects basic verification without contacting a keyring
- **AND** a protected root selects HMAC verification instead

### Requirement: Cached retrievals and acquisition check state have explicit record ownership
Cached retrieval material SHALL live under the owning source's working-data area. Persistent retrieval descriptors and managed acquisition check state SHALL have distinct typed records under index/<uuid>/index.json and participate in root membership, selected basic/keyring integrity and recoverable publication. Git working repositories SHALL remain provider-owned data, separately identified from acquired store/<entry-uuid>/ copies. Output revision reclaim SHALL not delete retrieval records or source material. Explicit retrieval purge SHALL coordinate source lifetimes without deleting independent acquired results or source registrations. Fresh-mode scratch and caller-owned destinations SHALL remain outside the persistent index and catalog.

#### Scenario: Output history is removed while source remains cached
- **WHEN** revision policy reclaims an old acquired output
- **THEN** its provenance remains and the source's retrieval cache remains independently available

#### Scenario: No-store operation completes
- **WHEN** an inline no_store retrieval and caller-owned acquisition succeed
- **THEN** the caller receives completed files without source, index, store, claim or catalog records being added to a Saucepan root
