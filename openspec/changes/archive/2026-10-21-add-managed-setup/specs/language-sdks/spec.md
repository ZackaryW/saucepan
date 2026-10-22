# Spec Delta

## ADDED Requirements

### Requirement: SDK setup delegates explicit administration to the manager
Both SDKs SHALL expose typed setup and setup-status methods and the optional exact self-update tag. Setup SHALL require an explicitly available manager, built shim and exact release tag, preserve caller-supplied operation/migration/digest inputs, and delegate root selection and orchestration to Rust. Installation, import and client construction SHALL remain free of native downloads, store initialization, PATH edits or keyring access. App-bound setup SHALL not discard identity to acquire administrative authority.

Setup result decoding SHALL validate its versioned schema, installation identities where applicable, status and exit 0/2 consistency separately from ordinary lifecycle operation decoding. Setup-status SHALL preserve observational states without treating absent/unmanaged as completed setup. Malformed results after possible mutation SHALL retain indeterminate transport failure semantics. Neither SDK SHALL automatically retry, migrate, reconcile, update or remove backups.

#### Scenario: Explicit setup through either SDK
- **WHEN** a local administrative client supplies a manager, shim, root and tag
- **THEN** it receives the same typed setup result and exit semantics as the CLI without language-owned policy expansion or publication

#### Scenario: Lost migration response
- **WHEN** the manager may have displaced a file before the SDK loses its response
- **THEN** the SDK reports indeterminate failure and preserves caller operation identity without retrying or restoring files

#### Scenario: Observe absent installation
- **WHEN** setup-status reports absent successfully
- **THEN** the SDK returns that observation without initializing or reporting managed readiness

#### Scenario: Shared tag update
- **WHEN** a caller explicitly supplies a self-update tag
- **THEN** the SDK passes it literally and preserves the existing request when the manager returns a failed operation
