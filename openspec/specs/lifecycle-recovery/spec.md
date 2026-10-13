# lifecycle-recovery Specification

## Purpose

Make application lifecycle progress observable and repeatable after failures, with bounded recovery and acceptance evidence spanning the whole lifecycle.

## Requirements

### Requirement: Lifecycle retries have stable operation identity
Multi-step lifecycle actions SHALL provide persistent operation identifiers and inspectable status, application identity, outcome and pending reasons. Reusing an operation identifier with the same inputs SHALL resume it or return its recorded terminal result without duplicate application or shim creation. Reusing it with different inputs SHALL fail before effects.

Statuses `preparing`, `commit_intent` and `cleanup_pending` SHALL denote unfinished work. Statuses `complete`, `failed` and `conflict` SHALL be terminal; another attempt after terminal failure or conflict SHALL require a new operation identifier. A recorded file-identity conflict MAY remain a pending reason while cleanup or maintenance awaits correction; it SHALL NOT be confused with terminal application-revision `conflict`.

A command returning a single lifecycle action result SHALL exit 0 for `complete` and 2 for a recorded unfinished, failed or conflicted outcome. Reconciliation SHALL exit 2 when any returned result is not `complete`, and 0 when all returned results are complete or there is no work to return. Preflight errors SHALL remain ordinary command errors. Read-only operation inspection SHALL report recorded status without claiming that inspecting an operation completed it; successfully reading a pending receipt SHALL NOT itself make inspection fail.

#### Scenario: The original response is lost
- **WHEN** a caller repeats a completed operation with the same identifier and inputs
- **THEN** it receives the recorded outcome without another application mutation

#### Scenario: Retry changes the requested shim or application
- **WHEN** an existing operation identifier is reused with different normalized inputs
- **THEN** it fails before altering state

### Requirement: Recovery preserves validated intent and isolates pending work
Opening a store SHALL recover committed journal transitions without network requests, probes, or waiting for a busy launcher. Required integrity checks for interrupted output publication SHALL remain mandatory as defined in [content-verification](../content-verification/spec.md). Store opening SHALL NOT resume lifecycle preparation or launcher maintenance. Explicit reconciliation SHALL retry only recorded committed work, rechecking file identity and containment. Uncommitted preparation SHALL require the original command to resume acquisition and validation. Filesystem cleanup pending for one app SHALL NOT prevent unrelated app operations.

#### Scenario: Restart during record publication
- **WHEN** execution stops before or after one of the records in a committed lifecycle transition is published
- **THEN** reopening completes a coherent metadata transition and preserves its operation identity

#### Scenario: Restart before validation finishes
- **WHEN** the original command retries a preparation operation after a crash before commit intent
- **THEN** candidate integrity and usability are checked again before activation
- **AND** inspection or reconciliation alone does not perform that preparation

#### Scenario: Busy cleanup does not poison the store
- **WHEN** one removal cannot delete its busy launcher
- **THEN** store opening and operations on other apps remain usable while reconciliation reports that pending cleanup

### Requirement: Lifecycle reconciliation preserves established access contracts
Lifecycle changes SHALL preserve immutable output identity, metadata-only routine launch, explicit full verification, native lease ownership, process-only lifetime markers and normal reclaim behavior. Reconciliation SHALL NOT create arbitrary claims, silently adopt unknown files, force-delete shared output content, or change source freshness rules.

#### Scenario: Reconciliation runs while a consumer holds a resource
- **WHEN** reconciliation completes removal of the consumer's former application metadata
- **THEN** the existing resource lease remains valid and reclaim still honors it

### Requirement: Completion requires integrated real-build acceptance
Verification SHALL exercise installation, successful and failed update, rollback, deactivation/reactivation, launcher maintenance, shared-output uninstall, pending cleanup, reconciliation and idempotent retry in one coherent lifecycle using actual built Saucepan binaries and shims. It SHALL include fault and concurrency tests around publication boundaries, preserve the existing mock installation, and report which host behaviors were actually executed.

#### Scenario: A complete simulated lifecycle finishes
- **WHEN** the isolated real-build acceptance sequence finishes
- **THEN** its report identifies build digests, operations, claims, bindings, owned files, pending outcomes and final retention results
- **AND** it demonstrates unchanged existing mock contents and no loss of live consumer resources

### Requirement: Recovery artifacts preserve record integrity and secret custody
Journals, prepared receipts, retired records and persisted temporary metadata SHALL carry integrity protection using the same mode as their owning store: checksums in basic mode and HMAC in keyring mode, binding their identities and generation. Claim metadata SHALL remain readable; bearer tokens and signing keys SHALL never be written into these artifacts. Store opening SHALL verify committed intent before replay, fail closed on corruption, and replay valid intent idempotently without client credentials, network access or new preparation. Preparation retries and operation inspection SHALL enforce applicable app credentials and action/scope restrictions. Missing or corrupt claim state SHALL NOT be interpreted as zero retention demand. Store catalogs, record writes and retirements SHALL participate in one coordinated recoverable transition.

#### Scenario: Inspect interrupted files
- **WHEN** an operation stops at a publication boundary
- **THEN** recovery metadata remains readable and integrity-verifiable but contains no bearer tokens or signing keys

#### Scenario: Tampered intent is replayed
- **WHEN** store opening encounters a changed committed transaction with an invalid checksum or HMAC for the selected store mode
- **THEN** it reports `integrity_error` without accepting the proposed claim or binding changes

#### Scenario: Valid intent survives a client exit
- **WHEN** an app commits intent and its client exits with its credential
- **THEN** recovery can complete that verified intent without reacquiring content or obtaining the client secret
