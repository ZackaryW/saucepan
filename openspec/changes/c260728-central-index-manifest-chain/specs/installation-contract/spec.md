## MODIFIED Requirements

### Requirement: Manifest-name installation identity
Saucepan SHALL use `sauce.name` from the resolved manifest as the installed identity for lookup, update, path, cat, and uninstall operations, regardless of which link of the manifest resolution chain supplied that manifest. The source target SHALL be retained as provenance rather than used as the installed identity, and the manifest source SHALL be retained alongside it.

#### Scenario: Repository and manifest names differ
- **WHEN** repository `owner/widget` declares manifest name `widget-cli`
- **THEN** the user addresses the installed sauce as `widget-cli` after installation and the index retains `owner/widget` as provenance

#### Scenario: Identity supplied by a central index
- **WHEN** repository `owner/widget` carries no manifest and a registered index describes it with manifest name `widget-cli`
- **THEN** the user addresses the installed sauce as `widget-cli`, and the index entry retains `owner/widget` as source provenance and the describing index as manifest provenance

#### Scenario: Manifest source is distinguishable
- **WHEN** a caller inspects two installed entries, one whose manifest came from its repository and one whose manifest came from an index
- **THEN** each entry reports which link supplied its manifest

### Requirement: Stable error categories
Saucepan SHALL map known missing targets or exhausted manifest resolution to exit code 1, Git/gh transport or execution failures to exit code 2, missing or invalid source configuration to exit code 3, origin conflicts to exit code 4, and unexpected failures to exit code 5.

#### Scenario: No sources are enabled
- **WHEN** a user runs install with no source sections enabled
- **THEN** Saucepan returns a configuration error with exit code 3

#### Scenario: Manifest resolution is exhausted
- **WHEN** a fetched target contains no configured root manifest and no registered index supplies one for it
- **THEN** Saucepan returns a not-found result with exit code 1 if no other source succeeds or fails operationally

#### Scenario: Configured manifest is absent but an index describes the target
- **WHEN** a fetched target does not contain the configured root manifest and a registered index supplies a manifest for it
- **THEN** Saucepan installs successfully rather than returning not found

#### Scenario: An unreachable index does not change the failure category
- **WHEN** a registered index cannot be fetched and manifest resolution is exhausted
- **THEN** Saucepan warns about the unreachable index and still returns a not-found result rather than a source error

#### Scenario: Git operation fails
- **WHEN** Git or gh cannot launch, authenticate, fetch, check out, or pull
- **THEN** Saucepan returns a source error with exit code 2 if no fallback source succeeds

#### Scenario: Installed origin conflicts
- **WHEN** an installation would replace the same manifest name from another origin
- **THEN** Saucepan returns a conflict with exit code 4
