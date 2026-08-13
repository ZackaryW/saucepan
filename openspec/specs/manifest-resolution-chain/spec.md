## Purpose

Define how Saucepan locates a fetched target's manifest: an ordered chain of sources consulted in precedence order, its termination and failure behavior, its extensibility, and the provenance it records.

## Requirements

### Requirement: Ordered manifest resolution
Saucepan SHALL resolve a fetched target's manifest by consulting an ordered chain of sources: the fetched target's own root manifest first, then registered central indexes. The first link that supplies a manifest SHALL win, and later links SHALL NOT be consulted.

#### Scenario: Repository supplies its own manifest
- **WHEN** a fetched target contains the configured root manifest
- **THEN** Saucepan uses it and consults no index

#### Scenario: Index supplies the manifest
- **WHEN** a fetched target contains no root manifest and a registered index describes that target
- **THEN** Saucepan uses the index-supplied manifest and the installation succeeds

#### Scenario: Repository manifest wins over an index
- **WHEN** a fetched target contains a root manifest and a registered index also describes that target
- **THEN** Saucepan uses the target's own manifest and ignores the index entry

### Requirement: Chain exhaustion preserves existing failure behavior
When no link in the chain supplies a manifest, Saucepan SHALL behave exactly as it does today for a missing manifest: a not-found result, subject to the existing multi-source fallback and error-precedence rules.

#### Scenario: No manifest anywhere
- **WHEN** a fetched target has no root manifest and no registered index describes it
- **THEN** Saucepan returns a not-found result with exit code 1 if no other source succeeds or fails operationally

#### Scenario: No indexes registered
- **WHEN** no index is registered and a fetched target has no root manifest
- **THEN** behavior is identical to the behavior before this change

### Requirement: The chain is additive to existing behavior
Introducing the chain SHALL NOT change the outcome of any operation that succeeds today. The chain SHALL only convert existing missing-manifest failures into successes.

#### Scenario: Existing installation is unaffected
- **WHEN** a target that installs successfully today is installed after this change
- **THEN** the resulting index entry, identity, and resolved commit are unchanged

### Requirement: The chain is extensible without a breaking change
The resolution order SHALL be defined as an ordered list of links rather than a fixed pair of checks, so that an additional link can be appended later without altering the precedence or outcome of existing links.

#### Scenario: Appending a future link
- **WHEN** an additional resolution link is introduced after the registered indexes
- **THEN** targets resolvable by the existing links resolve identically and only previously failing targets can reach the new link

### Requirement: Manifest source is recorded
Saucepan SHALL record which link of the chain supplied a manifest on the resulting index entry, and SHALL expose it through the machine-readable state commands.

#### Scenario: Reading manifest provenance
- **WHEN** a caller reads an installed entry whose manifest came from an index
- **THEN** the entry identifies the index as the manifest source

#### Scenario: Repository-sourced manifest provenance
- **WHEN** a caller reads an installed entry whose manifest came from the repository
- **THEN** the entry identifies the repository as the manifest source

### Requirement: Public install and update commands enforce the chain
The public install and update commands SHALL resolve manifests through the ordered repository-then-index chain rather than bypassing it at their composition boundary.

#### Scenario: Install resolves an index-supplied manifest
- **WHEN** a user installs a repository without a root manifest and a registered index supplies a valid manifest and ref
- **THEN** the public install command succeeds and records index manifest provenance

#### Scenario: Update resolves through the current chain
- **WHEN** a user updates an installed index-sourced sauce
- **THEN** the public update command consults the currently registered indexes after checking the repository link

### Requirement: Index-sourced update adopts current index policy
When update reaches the central-index link, Saucepan SHALL use the manifest and ref from the current winning registered index entry rather than retaining an index entry ref only because it was used during installation.

#### Scenario: Winning index entry changes after installation
- **WHEN** the winning registered index changes an installed target's manifest and ref
- **THEN** update checks out the new ref and records the newly resolved manifest and commit
