## MODIFIED Requirements

### Requirement: Stable error categories
Saucepan SHALL expose known missing targets or exhausted manifest resolution as exit code 1, Git/gh transport or execution failures as exit code 2, missing or invalid source configuration as exit code 3, origin conflicts as exit code 4, and unexpected failures as exit code 5 through its public command boundary.

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
