## Purpose

Define public update behavior for installed GitHub, custom-Git, local, and central-index-sourced sauces while preserving stored identity and failure categories.

## ADDED Requirements

### Requirement: Only installed remote sauces can be updated
Saucepan SHALL return not found for an unknown manifest name and SHALL reject update for a local-source entry without changing the local index.

#### Scenario: Unknown sauce
- **WHEN** a user updates a manifest name that is not installed
- **THEN** Saucepan returns not found and preserves the index

#### Scenario: Local sauce
- **WHEN** a user updates a sauce installed from a local source
- **THEN** Saucepan rejects the operation with a clear local-update error and preserves the index

### Requirement: Remote updates refresh installed metadata
Updating an installed GitHub or custom-Git sauce SHALL fetch its source again and replace the entry with the newly resolved manifest metadata while preserving its installed identity.

#### Scenario: Update a default-branch GitHub sauce
- **WHEN** the upstream manifest version advances and the user updates the installed sauce
- **THEN** Saucepan records the new version and resolved commit and reports success

#### Scenario: Update a custom-Git sauce
- **WHEN** a user updates an installed custom-Git sauce
- **THEN** Saucepan refreshes its manifest metadata and reports success

### Requirement: Stored GitHub refs are resolved again
Updating a GitHub entry with a stored requested ref SHALL resolve that same branch, tag, or commit again and refresh its resolved revision and manifest metadata.

#### Scenario: Update a ref-selected GitHub sauce
- **WHEN** a user updates a GitHub sauce with a stored requested ref
- **THEN** Saucepan resolves the same ref and records its current commit and manifest

### Requirement: Index-sourced updates re-read the current index
Updating a sauce whose manifest came from a central index SHALL re-read the currently registered index state and MAY adopt a newer entry ref supplied by that index.

#### Scenario: Central index advances an entry ref
- **WHEN** a registered index now supplies a newer ref and manifest for an installed index-sourced sauce
- **THEN** update checks out that ref and refreshes the installed manifest, resolved commit, and manifest provenance
