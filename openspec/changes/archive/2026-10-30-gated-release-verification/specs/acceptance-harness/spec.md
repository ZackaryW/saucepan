# Spec Delta

## ADDED Requirements

### Requirement: Artifacts mode tests packaged releases without building
Acceptance SHALL accept a directory of downloaded release assets and, in that mode, SHALL use the host platform's packaged manager and shim, the candidate's own installers and `SHA256SUMS`, and SHALL NOT invoke a compiler. The report SHALL record the candidate tag and the digest of every archive it used. Building from source SHALL remain the default when no artifacts are given.

#### Scenario: No compiler in artifacts mode
- **WHEN** acceptance runs with a candidate's assets on a machine without a Rust toolchain
- **THEN** every selected scenario runs against the packaged binaries and none attempts a build

#### Scenario: Setup uses the candidate installers
- **WHEN** the setup scenarios run in artifacts mode
- **THEN** the installer scripts and checksums they execute are byte-identical to the candidate's release assets

### Requirement: Release upgrade follows the store contract
A release upgrade scenario SHALL install the previous published release through setup and self-update it to the candidate through the provider fixture, determining both store contracts from the roots each release creates. With the same contract, the upgraded installation SHALL run store commands successfully; with different contracts, the scenario SHALL assert the documented `unsupported_store` break instead.

#### Scenario: Same-contract upgrade
- **WHEN** the previous release and the candidate share a store contract
- **THEN** self-update succeeds and the upgraded launcher queries the existing store

#### Scenario: Contract-changing upgrade
- **WHEN** the candidate introduces a new store contract
- **THEN** store commands after self-update fail with `unsupported_store`, the message names the reinstall procedure, and the previous root's files are unchanged

## MODIFIED Requirements

### Requirement: The integrated lifecycle remains one coherent scenario
The release-lifecycle scenario SHALL keep the integrated real-build lifecycle required by self-management and lifecycle-recovery as one coherent sequence. Splitting other behaviors into separate scenarios SHALL NOT remove any step of that sequence. Its two distinguishable builds SHALL be two locally stamped builds when building from source, and the candidate plus its verification twin in artifacts mode.

#### Scenario: Lifecycle scenario completes
- **WHEN** the release-lifecycle scenario runs
- **THEN** it bootstraps a packaged build through the fixture, updates to distinguishable bytes, preserves a failed candidate, rolls back, protects a busy launcher from reclaim and records the resulting identities

#### Scenario: Lifecycle in artifacts mode
- **WHEN** the scenario runs against a candidate and its verification twin
- **THEN** it updates from one to the other without building, and the report records both digests

### Requirement: Hosted checks run acceptance in each launch environment
Release verification SHALL run acceptance in artifacts mode on Windows from PowerShell, on Windows from Git Bash, on macOS and on Linux, each producing its own report. The manual platform checks SHALL run acceptance from source in the same environments, keep their manual or explicitly invoked triggers, and let every platform finish rather than cancelling on the first failure.

#### Scenario: Launch-environment-specific failure
- **WHEN** a defect only reproduces when the manager is launched from Git Bash
- **THEN** the Git Bash acceptance job fails while the PowerShell job may pass, and both reports identify their environment
