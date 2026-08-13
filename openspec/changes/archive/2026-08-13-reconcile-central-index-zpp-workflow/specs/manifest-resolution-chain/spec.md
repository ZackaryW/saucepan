## ADDED Requirements

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
