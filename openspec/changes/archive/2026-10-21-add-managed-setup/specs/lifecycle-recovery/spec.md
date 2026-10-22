# Spec Delta

## ADDED Requirements

### Requirement: Setup preparation has durable identity and truthful results
Setup SHALL persist normalized inputs, selected root identity, reserved registration identities, expanded policy evidence, required staged inputs, linked installation operation and migration evidence before their corresponding effects. Once store-backed, these records SHALL participate in the owning store's integrity and membership contract and contain no secrets. A same-operation retry SHALL reuse the original registrations and expanded inputs; altered tag, shim identity, migration inputs or root identity SHALL conflict before dependent effects. Setup SHALL preserve the established preparing/commit-intent/complete/failed/conflict semantics and distinguish preparation from committed installation work. Initialization errors before a setup receipt exists SHALL remain initialization errors.

Setup results SHALL include a schema version, operation, status, changed flag, root, applicable installation identities, linked lifecycle operation, retained backup locations and actionable pending/error information. Complete SHALL exit 0; a recorded noncomplete result SHALL exit 2. Preflight errors SHALL retain ordinary nonzero diagnostics. Retrying a terminal failed/conflict operation SHALL not turn it into a new attempt. New attempts SHALL retain earlier evidence and explicitly verify any backup they reuse.

#### Scenario: Registration succeeds before interruption
- **WHEN** setup stops after source or policy registration but before bootstrap completes
- **THEN** retrying its operation reuses those identities rather than generating a different request or duplicate application

#### Scenario: Script temporary directory is removed
- **WHEN** an installer exits after setup has persisted preparation or commit intent
- **THEN** durable inputs and backup information needed for the documented retry remain available outside script scratch

#### Scenario: Changed retry inputs
- **WHEN** a pending setup operation is retried with another tag or shim digest
- **THEN** the original receipt remains unchanged and the conflicting request is rejected

#### Scenario: Corrupt setup evidence
- **WHEN** store-backed setup intent fails integrity validation
- **THEN** setup/recovery fails without accepting registrations, migration permission or backup identity from that intent

### Requirement: Setup recovery preserves existing lifecycle boundaries
Setup and migration SHALL serialize competing work for the same root/launcher, retain the existing lifecycle revision and ownership fences, and use explicit retries for uncommitted preparation. Store opening and inspection SHALL not download, probe, displace files or resume preparation. Existing reconciliation SHALL complete only verified committed publication/maintenance. Pending migration or setup SHALL not block unrelated applications beyond required short coordination. Interrupted or failed setup SHALL report whether the original launcher is still available or only a preserved backup is available; it SHALL not promise atomic rollback of raw-file migration.

#### Scenario: Concurrent first setup
- **WHEN** two setup requests target the same root and launcher
- **THEN** at most one installation is published and the other observes reuse, pending work or conflict without overwriting files

#### Scenario: Retry after original file displacement
- **WHEN** setup stops after preserving and displacing the verified raw manager but before publishing the shim
- **THEN** the original backup remains available and retry checks recorded identities before continuing

#### Scenario: Inspection of pending setup
- **WHEN** a caller inspects an incomplete setup operation
- **THEN** it receives the actual recorded state without causing preparation, migration or publication
