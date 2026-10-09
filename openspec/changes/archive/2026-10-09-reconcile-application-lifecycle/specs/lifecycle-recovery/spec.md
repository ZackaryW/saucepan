# Spec Delta

## Purpose

Make application lifecycle progress observable and repeatable after failures, with bounded recovery and acceptance evidence spanning the whole lifecycle.

## ADDED Requirements

### Requirement: Lifecycle retries have stable operation identity
Multi-step lifecycle actions SHALL provide persistent operation identifiers and inspectable status, application identity, outcome and pending reasons. Reusing an operation identifier with the same inputs SHALL resume it or return its recorded terminal result without duplicate application or shim creation. Reusing it with different inputs SHALL fail before effects. Pending/conflicted CLI results SHALL use nonzero exit status and SHALL NOT imply completed success.

#### Scenario: The original response is lost
- **WHEN** a caller repeats a completed operation with the same identifier and inputs
- **THEN** it receives the recorded outcome without another application mutation

#### Scenario: Retry changes the requested shim or application
- **WHEN** an existing operation identifier is reused with different normalized inputs
- **THEN** it fails before altering state

### Requirement: Recovery preserves validated intent and isolates pending work
Opening a store SHALL recover committed metadata without network requests, probes, or waiting for a busy launcher. Explicit reconciliation SHALL retry only recorded work, rechecking file identity and containment. Uncommitted preparation SHALL NOT become an active application without validation. Filesystem cleanup pending for one app SHALL NOT prevent unrelated app operations.

#### Scenario: Restart during record publication
- **WHEN** execution stops before or after one of the records in a committed lifecycle transition is published
- **THEN** reopening completes a coherent metadata transition and preserves its operation identity

#### Scenario: Restart before validation finishes
- **WHEN** a preparation operation is retried after a crash before commit intent
- **THEN** candidate integrity and usability are checked again before activation

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

