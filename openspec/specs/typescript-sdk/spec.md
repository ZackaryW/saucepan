## Purpose

Provide a typed Node.js adapter to Saucepan's central-store CLI while keeping acquisition, storage, policy, and authority decisions in the Rust core.

## Requirements

### Requirement: Typed central-store operations
The SDK SHALL provide asynchronous methods for initialization, app registration/configuration, acquisition, scoped view retrieval/verification, path lookup, mirroring, history, retained snapshot reads, and shared executable lookup. Public types SHALL represent the current JSON protocol, including all three providers and nullable lookup results.

#### Scenario: Acquire a Git folder
- **WHEN** an app acquires two folders from the same Git origin and ref
- **THEN** both results retain the core's shared source identity and expose their selected artifact paths

#### Scenario: Consume a package
- **WHEN** a Node.js 20+ ESM consumer installs the built package
- **THEN** it can import the client and its declarations without runtime dependencies beyond Node.js and the separately supplied CLI

### Requirement: Preserve caller context without duplicating core policy
The SDK SHALL accept an app name, marker file, or registration token and forward calls to the core. It SHALL support centrally persisted settings/filters without local TOML, implicit registration, SDK-owned history, permission rules, or token refresh. Returned token and view format versions SHALL be checked.

#### Scenario: Verify a saved view
- **WHEN** settings change after an app saves a view
- **THEN** verifying that saved view exposes the core's failure while the registration token remains usable

### Requirement: Literal arguments and bounded process execution
The SDK SHALL invoke an independently supplied executable using argument arrays without a shell, defaulting to the user-level `.saucepan/bin` path. It SHALL support configurable timeout and output-buffer limits and retain process failure diagnostics without copying command text into generated error messages.

#### Scenario: Literal app name
- **WHEN** an app name contains shell metacharacters or starts with a hyphen
- **THEN** the core receives that literal name rather than a command or CLI option

#### Scenario: Timeout
- **WHEN** a call exceeds its configured timeout
- **THEN** it rejects with process failure information and releases its temporary request files

### Requirement: Isolated request inputs and explicit test stores
The SDK SHALL create separate temporary JSON inputs for each call and clean them up after success or failure. A test store SHALL require both a nonempty root and an explicit 32-byte hexadecimal key; invalid configuration SHALL fail without falling back to the user store.

#### Scenario: Concurrent proof calls
- **WHEN** calls share a registration token and run concurrently
- **THEN** each owns its request files, caller-owned markers remain unchanged, and files are removed after completion
