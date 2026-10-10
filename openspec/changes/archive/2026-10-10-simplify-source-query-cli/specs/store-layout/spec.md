## ADDED Requirements

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
