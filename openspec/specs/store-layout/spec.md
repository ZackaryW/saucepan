# store-layout Specification

## Purpose

Separate source working data, acquisition metadata, and stored outputs so their lifecycles remain independent and applications can resolve content by UUID.

## Requirements

### Requirement: Source metadata and output content have separate locations
Saucepan SHALL keep source definitions under `source/<source-uuid>.json`, optional provider working data under `source/<source-uuid>/`, live application and output metadata under `index/<uuid>/index.json`, and available acquired content under `store/<entry-uuid>/`. App and output records SHALL have distinguishable record types. Completed uninstall SHALL retire the live app record while retaining its operation receipt as specified by [application-lifecycle](../application-lifecycle/spec.md); output reclaim SHALL retain provenance as specified by [output-policies](../output-policies/spec.md). UUID and path validation SHALL prevent access outside the owning root.

#### Scenario: Application and output records coexist
- **WHEN** an application acquires content from a registered source
- **THEN** its app index references an output index whose entry UUID resolves to the corresponding store directory
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
Saucepan SHALL use a newly defined data contract without inheriting the old FORMAT_VERSION or compatibility decoders. Opening a legacy store SHALL fail explicitly without changing its data.

#### Scenario: An old encrypted central index is present
- **WHEN** initialization or opening detects a legacy Saucepan store
- **THEN** it reports that a separate new-format root is required and leaves the legacy store unchanged

### Requirement: An explicit root isolates the full application flow
Saucepan SHALL accept an explicit root for initialization and subsequent operations, and SHALL preserve that root through shim launch and child application execution. Explicit-root operations SHALL NOT silently read or mutate the production root. The repository's `mock/` directory SHALL be usable directly as a simulated `.saucepan` root without an additional nested `.saucepan` directory.

#### Scenario: The simulated installation launches its managed executable
- **WHEN** the caller invokes the shim from `mock/bin/` with the explicit mock root
- **THEN** source, index, store, claim, activation, and reclaim operations resolve within that root
- **AND** the user's production installation is not used as a fallback
