## Purpose

Define the public configuration-loading, validation, source-activation, and executable-override behavior relied on by Saucepan commands.

## Requirements

### Requirement: Configuration file is required and validated
Saucepan SHALL require a readable `saucepan.toml` at the selected workspace root and SHALL reject invalid TOML with a configuration error.

#### Scenario: Missing configuration
- **WHEN** a user invokes a command in a workspace without `saucepan.toml`
- **THEN** Saucepan fails with a configuration error that names `saucepan.toml`

#### Scenario: Invalid configuration
- **WHEN** `saucepan.toml` contains invalid TOML
- **THEN** Saucepan fails with a configuration error identifying the invalid file

### Requirement: Empty configuration is valid
An empty `saucepan.toml` SHALL be valid and SHALL enable no artifact source.

#### Scenario: Read-only command with empty configuration
- **WHEN** a user runs `list` with an empty `saucepan.toml`
- **THEN** the command succeeds with no source enabled

### Requirement: Source sections activate their sources
The presence of a supported source section SHALL activate that source for commands that consult sources.

#### Scenario: Local section activates local lookup
- **WHEN** `saucepan.toml` contains a `[local]` section and the user installs a name
- **THEN** Saucepan consults the local installed index before remote sources

### Requirement: Configured jq executable is honored
When `saucepan.toml` declares a `jq` executable, search SHALL invoke that value instead of the default `jq` command.

#### Scenario: Custom jq executable
- **WHEN** a user searches in a workspace whose configuration declares a custom `jq` path
- **THEN** Saucepan invokes the configured executable
