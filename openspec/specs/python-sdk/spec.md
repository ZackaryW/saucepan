# python-sdk Specification

## Purpose

Define a vendorable, standard-library-only Python client for the central-store CLI, preserving app context, acquisition results, and process failures without duplicating core policy.

## Requirements

### Requirement: Central-store client and complete command coverage
The Python SDK SHALL expose `Saucepan` with methods for initialization, app registration/configuration, acquisition, view/verification, artifact paths, mirrors, history, retained snapshot reads, and shared-executable lookup. Results SHALL preserve the CLI's single JSON document as Python values, including nullable path/history results. Artifact paths SHALL come from the CLI rather than reconstructed store layout.

#### Scenario: Acquire and locate content
- **WHEN** a registered app acquires a Git, URL, or local recipe through Python
- **THEN** it receives the actual artifact, central directory, and verification/fallback status, and can use the returned IDs for subsequent commands

#### Scenario: Exact history and folder selection
- **WHEN** the app reads a retained snapshot with an optional folder
- **THEN** the SDK forwards the source/snapshot IDs and folder and returns the CLI result without advancing current or assigning a separate source identity

#### Scenario: Nullable lookup
- **WHEN** the CLI returns JSON null for an absent path or history
- **THEN** the method returns `None` rather than inventing a path or raising an obsolete not-found category

### Requirement: Registered app context and central settings
The client SHALL accept ordinary app identifiers or authenticated registration tokens/marker paths and SHALL support creating an independent caller context through `for_app()`. Token and marker inputs SHALL be mutually exclusive. Registration and configuration SHALL forward optional settings/filters as JSON. The SDK SHALL NOT read workspace TOML, cache app views/settings, infer acquisition permissions from filters, or rewrite caller-owned markers.

#### Scenario: Stable caller token
- **WHEN** app settings or acquired entries change
- **THEN** the original token remains usable and subsequent view calls observe the current centrally stored state

#### Scenario: Context isolation
- **WHEN** a client selects another app with `for_app()`
- **THEN** executable, timeout, and test-store options are retained while the old app/token/marker and authoritative flag are replaced

#### Scenario: Independent token input
- **WHEN** the caller mutates a token dictionary after constructing the client
- **THEN** that mutation does not alter the client context already constructed

### Requirement: Supplied executable and explicit test mode
The SDK SHALL use a supplied executable verbatim or default to the user-level `.saucepan/bin/saucepan` executable with the platform suffix. It SHALL NOT acquire or build binaries at runtime. Isolated mode SHALL require both a custom root and a 64-character hexadecimal test key, with no automatic fallback from production keyring failures.

#### Scenario: Shared executable default
- **WHEN** no binary option is supplied
- **THEN** the client selects the user-level shared executable without consulting workspace configuration

#### Scenario: Incomplete test configuration
- **WHEN** only a test root or key is provided, or the key is malformed
- **THEN** construction fails before invoking the CLI

### Requirement: Safe request and process lifecycle
Calls SHALL use argument arrays without a shell, with per-request temporary JSON files removed after success or failure. Token files SHALL be created with private permissions supported by the OS. Concurrent calls SHALL NOT share mutable request files. A configurable positive timeout SHALL terminate a timed-out child; the default SHALL have no deadline.

#### Scenario: Literal arguments
- **WHEN** app names or paths contain spaces, Unicode, shell characters, or leading dashes
- **THEN** they reach the CLI as literal arguments rather than shell syntax or unintended options

#### Scenario: Concurrent calls and cleanup
- **WHEN** calls run concurrently or fail
- **THEN** each owns its request files and cleans them up without deleting another request's files or caller-owned markers

### Requirement: Current errors and response versions
Nonzero CLI exits SHALL raise `SaucepanError` carrying the actual exit code, stdout, and stderr without applying the retired workspace category mapping. Launch failures and timeouts SHALL carry no exit code. Invalid JSON and unsupported token/view versions SHALL fail explicitly. Process-error messages SHALL NOT reproduce secret-bearing command lines.

#### Scenario: CLI failure diagnostics
- **WHEN** the CLI exits with an operational or usage error
- **THEN** the exception preserves its actual status and diagnostic streams

#### Scenario: Missing executable or timeout
- **WHEN** the executable cannot launch or exceeds the timeout
- **THEN** the SDK reports a process failure without exposing the command's test key

#### Scenario: Unsupported response
- **WHEN** registration or view output carries an unsupported format version, or successful stdout is not JSON
- **THEN** the SDK rejects the response rather than silently consuming it

### Requirement: Standalone distribution and real client verification
The package SHALL support Python 3.9 and newer, import only the standard library at runtime, and remain independently vendorable. Development dependencies SHALL remain separate from runtime dependencies, and SDK files SHALL remain excluded from the Rust crate. Python tests and independently runnable behavior scenarios SHALL exercise the public client against the actual central-store CLI with isolated test roots/keys. Process-boundary tests MAY use separate real child processes to produce timeout or malformed-output conditions.

#### Scenario: Vendored client
- **WHEN** only the SDK runtime and a supplied CLI are available with Python site packages disabled
- **THEN** the client can import, register an app, and read its view

#### Scenario: Runtime protocol regression
- **WHEN** CLI arguments, JSON results, or error behavior diverge from the adapter
- **THEN** the real-client suite fails instead of treating direct CLI fixtures as SDK compatibility evidence

### Requirement: Explicit migration from the workspace API
Python SDK 0.5.0 SHALL replace `Workspace`, `Sauce`, `Bucket`, and their legacy error classes with the central-store client. Documentation SHALL state that existing callers require migration and describe registration, declarative recipes, central configuration, and views. The SDK SHALL NOT add wrappers for removed bucket/install/update/uninstall commands.

#### Scenario: Existing Python caller migrates
- **WHEN** a caller upgrades from the workspace SDK
- **THEN** the documentation directs it to `Saucepan` and the current operation surface without promising compatibility for old signatures or error categories
