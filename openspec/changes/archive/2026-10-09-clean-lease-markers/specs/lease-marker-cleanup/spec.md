# Spec Delta

## Purpose

Prevent resource lookups from accumulating lifetime bookkeeping while preserving native protection for running processes and safely removing abandoned markers.

## ADDED Requirements

### Requirement: Non-process access does not accumulate lifetime markers
Metadata path lookup, full verification, and native resource leases SHALL NOT create per-use process lifetime marker files. Metadata lookup SHALL retain its current identity, readiness, containment and zero-payload-read checks. Full checks and native guards SHALL remain protected against reclaim for their documented lifetimes.

#### Scenario: Repeated access to a retained output
- **WHEN** a caller repeatedly resolves a path, fully verifies content, or obtains and drops native entry/claim guards
- **THEN** these operations create no process lifetime marker files
- **AND** full checks and live guards continue to defer reclaim

### Requirement: Dead markers can be cleaned independently of retention
Reclaim SHALL remove recognized closed lifetime markers even when their output is retained or actively bound. It SHALL preserve live native ownership and unrelated files, and SHALL NOT use PID or elapsed time as proof of death. Cleanup SHALL coordinate with marker creation and leave busy entries untouched until a later pass.

#### Scenario: Old lookups left markers on a retained output
- **WHEN** reclaim runs after all handles to those markers have closed
- **THEN** the stale marker files are removed without changing the output's retention or provenance

#### Scenario: A marker is still held or creation is in progress
- **WHEN** reclaim encounters native ownership or entry coordination held by a launcher or consumer
- **THEN** it preserves the marker and retries on a later reclaim operation

### Requirement: Launch ownership survives cleanup and wrapper failure
Managed shims and leased runs SHALL establish inherited native ownership before starting the target and retain uninterrupted reclaim protection throughout the handoff. Cleanup SHALL remain compatible with existing launcher markers. A failed launch SHALL leave no live ownership, and any closed marker it leaves SHALL be removable by reclaim.

#### Scenario: Wrapper dies before its target
- **WHEN** a wrapper terminates while the target retains its inherited lifetime handle
- **THEN** reclaim preserves its marker and output until that native ownership ends

#### Scenario: Process exits while its output remains retained
- **WHEN** reclaim runs after the target and any inheriting descendants release the lifetime handle
- **THEN** it removes the closed marker while preserving otherwise retained output content
