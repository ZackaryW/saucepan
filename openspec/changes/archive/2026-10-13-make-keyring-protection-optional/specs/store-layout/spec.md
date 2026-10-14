## MODIFIED Requirements

### Requirement: Source metadata and output content have separate locations
Saucepan SHALL keep source definitions under `source/<source-uuid>.json`, optional provider working data under `source/<source-uuid>/`, live application and output metadata under `index/<uuid>/index.json`, application claim registries under `claims/<app-uuid>/index.json`, separate claim envelopes under `claims/<app-uuid>/<claim-uuid>.json`, and available acquired content under `store/<entry-uuid>/`. Claims and registered policies SHALL NOT remain embedded as a second mutable copy in the application index. Claim JSON files SHALL contain readable payloads verified using the store's basic checksums or keyring-backed HMAC tags as specified by claim-security. App and output records SHALL have distinguishable record types. Completed uninstall SHALL retire the live app record while retaining its operation receipt as specified by [application-lifecycle](../application-lifecycle/spec.md); output reclaim SHALL retain provenance as specified by [output-policies](../../../../../specs/output-policies/spec.md). UUID and path validation SHALL prevent access outside the owning root.

#### Scenario: Application and output records coexist
- **WHEN** an application acquires content from a registered source
- **THEN** its app index identifies its claim registry, whose claim references an output index with an entry UUID resolving to the corresponding store directory
- **AND** the application does not require a store directory of its own

### Requirement: Read commands do not implicitly initialize a missing store
`saucepan init` SHALL initialize the resolved default or explicit root in basic mode unless `--integrity keyring` is explicitly selected. `--integrity basic` SHALL explicitly select the default. Native creation SHALL provide the same default and explicit choice. Successful initialization SHALL identify the selected mode. Invalid mode values SHALL fail before initialization effects. Other commands encountering a missing root directory SHALL fail with `unsupported_store`, no success stdout, and guidance to initialize the selected root; they SHALL NOT create it or its signing key. Existing corrupt, legacy or incomplete stores SHALL retain existing error handling without an implicit reset. Help and version SHALL succeed without home lookup or store initialization.

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
