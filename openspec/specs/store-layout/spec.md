# store-layout Specification

## Purpose

Separate source working data, acquisition metadata, and stored outputs so their lifecycles remain independent and applications can resolve content by UUID.

## Requirements

### Requirement: Source metadata and output content have separate locations
Saucepan SHALL keep source definitions under `source/<source-uuid>.json`, optional provider working data under `source/<source-uuid>/`, live application and output metadata under `index/<uuid>/index.json`, application claim registries under `claims/<app-uuid>/index.json`, separate claim envelopes under `claims/<app-uuid>/<claim-uuid>.json`, and available acquired content under `store/<entry-uuid>/`. Claims and registered policies SHALL NOT remain embedded as a second mutable copy in the application index. Claim JSON files SHALL contain readable payloads with HMAC integrity tags as specified by claim-security. App and output records SHALL have distinguishable record types. Completed uninstall SHALL retire the live app record while retaining its operation receipt as specified by [application-lifecycle](../application-lifecycle/spec.md); output reclaim SHALL retain provenance as specified by [output-policies](../output-policies/spec.md). UUID and path validation SHALL prevent access outside the owning root.

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
Saucepan SHALL use a newly defined data contract without inheriting the old FORMAT_VERSION or compatibility decoders. Opening a legacy store or an earlier inline-claim layout SHALL fail explicitly with `unsupported_store` without changing its data. New stores SHALL have an explicit store UUID and contract discriminator. Missing or corrupt required claim metadata SHALL NOT be treated as an empty new store.

#### Scenario: An old encrypted central index is present
- **WHEN** initialization or opening detects a legacy Saucepan store
- **THEN** it reports that a separate new-format root is required and leaves the legacy store unchanged

### Requirement: An explicit root isolates the full application flow
Saucepan SHALL accept an explicit root for initialization and subsequent operations, and SHALL preserve that root through its managed Saucepan shim and child Saucepan execution. Explicit-root operations SHALL NOT silently read or mutate the production root. A fresh simulated root SHALL be usable directly without an additional nested `.saucepan` directory. The repository's existing `mock/` is historical evidence under an earlier contract; current acceptance SHALL preserve it and use a separate fresh root rather than treating explicit root selection as migration permission.

#### Scenario: The simulated installation launches its managed executable
- **WHEN** the caller invokes the shim from a fresh simulated root's `bin/` directory with that explicit root
- **THEN** source, index, store, claim, activation, and reclaim operations resolve within that root
- **AND** the user's production installation is not used as a fallback

### Requirement: CLI operations default to the user-home store
When `--root` is omitted, CLI operations SHALL resolve the operating-system user's home directory and append `.saucepan`. On Windows the default SHALL use USERPROFILE and on Unix HOME, with the standard operating-system home lookup fallback. An explicit `--root PATH` SHALL take precedence without requiring home resolution; relative overrides SHALL remain relative to the working directory. The CLI SHALL NOT search working-directory ancestors or fall back to another store after an error. Unavailable, empty or relative home resolution SHALL produce `invalid_request` with guidance to pass `--root`. Native APIs and shim-carried explicit roots SHALL retain their existing contracts.

#### Scenario: Default location is independent of working directory
- **WHEN** a caller omits --root while working in a project with a local .saucepan folder
- **THEN** the operation uses the user-home .saucepan store
- **AND** the project-local store is not read or modified

#### Scenario: Override remains isolated
- **WHEN** an explicit root is supplied and the default home store exists or home lookup is unavailable
- **THEN** only the explicit root is used

### Requirement: Read commands do not implicitly initialize a missing store
`saucepan init` SHALL initialize the resolved default or explicit root. Other commands encountering a missing root directory SHALL fail with `unsupported_store`, no success stdout, and guidance to initialize the selected root; they SHALL NOT create it or its signing key. Existing corrupt, legacy or incomplete stores SHALL retain existing error handling without an implicit reset. Help and version SHALL succeed without home lookup or store initialization.

#### Scenario: First query precedes initialization
- **WHEN** a caller runs saucepan query-sources before the default root exists
- **THEN** the command fails with guidance to run saucepan init
- **AND** the root remains absent

#### Scenario: Initialization followed by querying
- **WHEN** a caller initializes a disposable user-home store and then queries without --root
- **THEN** both commands use the same .saucepan directory
