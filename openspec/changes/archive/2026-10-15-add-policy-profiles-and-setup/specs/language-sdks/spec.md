# Spec Delta

## MODIFIED Requirements

### Requirement: Typed operations preserve the current resource contract
Both SDKs SHALL provide equivalent typed operations for explicit initialization/application configuration, source registration/discovery/visibility, record inspection, immutable policy registration/lookup/adoption, scoped claim acquisition/query/context/update/recovery/release, retrieval and acquisition, exact paths and verification, reclaim/purge, registered group operations, pins and guarded execution. They SHALL additionally expose Rust-owned profile listing/expansion, effective-policy explanation and explicit managed install, bootstrap, update-app, self-update, rollback, uninstall, operations inspection and reconciliation. Requests SHALL accept objects without caller-authored JSON files. Policy UUID references, explicit portable catalogs, omitted optional values, exact output identities and provenance SHALL retain Rust CLI meaning. SDKs SHALL NOT evaluate policies, infer source aliases or latest outputs, reconstruct store paths, decode earlier schemas or provide legacy method adapters. Standalone shim install/replace/adopt and executable provisioning SHALL remain excluded and documented as such.

#### Scenario: Query a registered policy
- **WHEN** either language looks up a known policy UUID
- **THEN** it returns that exact immutable definition using the current policy command without adopting or modifying it

#### Scenario: Optional policy omitted
- **WHEN** a caller omits acquisition verification, identity or pin options
- **THEN** the SDK does not insert policy objects, create an app or pin, or replace omitted fields with null/default values

#### Scenario: Expand before explicit installation
- **WHEN** a caller requests an installation profile through either SDK
- **THEN** Rust returns equivalent flat definitions and refs without registration, fetching or installation
- **AND** the caller separately registers required definitions and supplies the installation request, built shim and explicit operation inputs

## ADDED Requirements

### Requirement: SDK lifecycle results distinguish completion from pending work
Lifecycle SDK operations SHALL decode documented receipt responses on exit 0 and exit 2, preserving typed operation state and the observed command exit code. Complete receipt/exit combinations SHALL be validated separately from pending, failed and conflict results; operations inspection SHALL retain its read-only command semantics. Reconciliation lists SHALL preserve each operation's status. Domain error envelopes SHALL remain Saucepan errors and malformed or inconsistent receipts SHALL be protocol errors, with mutation uncertainty when execution might have occurred. Other operations SHALL retain their current exit handling. The SDK SHALL NOT automatically retry, reconcile, activate, release retention or compensate for a non-complete result.

#### Scenario: Windows launcher cleanup remains pending
- **WHEN** uninstall returns a valid cleanup-pending receipt with exit 2
- **THEN** both SDKs return the pending operation and exit code without claiming removal completed or treating valid receipt JSON as a malformed domain error

#### Scenario: Malformed lifecycle response
- **WHEN** a lifecycle command might have run but emits an invalid or exit-inconsistent receipt
- **THEN** the SDK reports a potentially indeterminate protocol failure rather than retrying or fabricating completion

### Requirement: Profiles preserve SDK identity and content lifetime boundaries
SDK profile calls SHALL delegate recipe expansion to Rust without maintaining language-specific policy tables. Managed setup SHALL require an explicitly available manager and caller-provided built shim. Client construction and package installation SHALL remain side-effect-free. Profile use SHALL NOT invent app scope, register identity, enable credentials, fall back after denial or create/release pins. Temporary-profile results SHALL use existing guarded execution or caller-owned delivery; the SDK SHALL NOT return a temporary path as though its native lifetime survived command exit.

#### Scenario: Temporary command consumes material
- **WHEN** a caller uses a temporary profile with a guarded run operation
- **THEN** the child consumes material within its native lifetime and SDK disposal does not claim an independent retained handle

#### Scenario: Bound setup operation is administrative
- **WHEN** an app-bound client attempts a setup or registration command reserved for local administration
- **THEN** the operation is rejected without silently dropping the app context
