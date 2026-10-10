# application-lifecycle Specification

## Purpose

Provide consistent application installation, updates, rollback and removal while preserving independent claims, immutable cached outputs and live consumers.

## Requirements

### Requirement: Managed installation publishes a complete usable application
Installation SHALL publish the initial managed claim, validated binding and owned launcher as one recoverable lifecycle operation. A successful result SHALL identify the application, claim, output, shim and operation. Failure before commit intent SHALL NOT leave a live partially installed application. Source discovery and independently cached output changes SHALL be reported separately from application success.

#### Scenario: Initial candidate fails its probe
- **WHEN** installation acquires a candidate that fails validation or its bounded probe
- **THEN** it reports failure without publishing a live application, selected claim or active launcher
- **AND** any cached output remains subject to normal retention

#### Scenario: Installation is interrupted during publication
- **WHEN** installation has recorded commit intent but stops before completing launcher and application publication
- **THEN** retry or reconciliation uses the same reserved identities and completes or reports the remaining work without creating a second application
- **AND** a partially published launcher cannot run an unvalidated application

### Requirement: Managed updates and rollback commit claim and binding together
Managed update and self-update SHALL preserve the previous selected claim and binding until candidate validation and pre/post-probe integrity verification succeed. Managed rollback SHALL select an available validated output belonging to the chosen claim's item without changing its original acquisition request. Successful managed transitions SHALL commit that claim selection and binding together. The candidate SHALL remain protected against reclaim throughout preparation and commit.

#### Scenario: Failed self-update preserves both selections
- **WHEN** a downloaded self-update candidate fails its probe
- **THEN** the previous claim and active binding remain selected and subsequent shim launches still use the previous output

#### Scenario: Rollback selects a retained output
- **WHEN** managed rollback selects a retained valid output for the selected claim's item
- **THEN** the claim and binding both select that output without remote discovery
- **AND** other claims, settings and shims remain unchanged

#### Scenario: Invalid rollback target
- **WHEN** rollback selects a reclaimed, corrupt, incompatible or different-item output
- **THEN** the operation fails without moving the claim or binding

#### Scenario: Reclaim races candidate preparation
- **WHEN** another acquisition or reclaim occurs after candidate publication but before its activation commit
- **THEN** the prepared candidate remains protected through verification, probing and commit

#### Scenario: A committed installation intent survives process exit
- **WHEN** the preparing process exits after recording commit intent but before application publication
- **THEN** reclaim reports the candidate as protected by that pending intent until reconciliation completes or terminally fails it
- **AND** reconciliation verifies integrity before activating the candidate

### Requirement: Primitive operations retain their distinct meaning
Resource claim updates SHALL NOT implicitly activate an executable. Binding-only activation SHALL NOT move a claim. Deactivation SHALL remain idempotent, clear only the active binding, preserve claims and running consumers, and perform no content reclaim. A later binding-only activation commit SHALL still be permitted to restore an inactive application.

#### Scenario: Deactivate and later reactivate
- **WHEN** an application is deactivated while a consumer already holds a lease
- **THEN** that consumer continues, new launches fail until reactivation, and claims remain intact
- **AND** normal activation can reuse its installed shim

### Requirement: Concurrent lifecycle operations cannot publish stale or removed applications
Managed update and rollback SHALL detect intervening application mutation and report a terminal `conflict` for that operation identifier rather than overwrite newer intent. A new attempt SHALL use a new operation identifier, as defined in [lifecycle-recovery](../lifecycle-recovery/spec.md). Removal SHALL prevent any in-flight acquisition, activation, installation or maintenance from restoring the removed registration. Unrelated applications SHALL remain operable while one operation is pending.

#### Scenario: Deactivation overtakes managed update
- **WHEN** deactivation commits after a managed update captured its application state but before that update commits
- **THEN** the managed update reports a conflict and the application remains inactive
- **AND** this does not prohibit a later explicit binding-only activation

#### Scenario: Uninstall overtakes candidate probing
- **WHEN** uninstall logically removes an application while an earlier candidate probe is running
- **THEN** that candidate cannot subsequently recreate a claim, binding or live registration

### Requirement: Uninstall separates logical removal from physical cleanup
Uninstall SHALL release the application's claims, clear its binding, reject new app mutation and launch, and remove its owned shims and live application record recoverably. Inspection and reconciliation SHALL remain available. Busy or changed files SHALL leave `cleanup_pending` with an inspectable reason without killing consumers or deleting unknown files. Known completed removal SHALL be idempotent; an unrelated unknown UUID SHALL fail. Uninstall SHALL NOT delete sources, source working data, output provenance or cached output content.

#### Scenario: Self-uninstall through a busy Windows launcher
- **WHEN** Saucepan logically removes itself while its launcher remains running
- **THEN** new launches fail and file cleanup can remain pending with an operation identifier
- **AND** reconciliation using a directly invoked binary after launcher exit can complete removal

#### Scenario: Shared outputs outlive registration
- **WHEN** an app is uninstalled while another app or live consumer uses the same output
- **THEN** that other app and consumer remain usable
- **AND** later reclaim respects surviving output policies, claim demand, active bindings and leases

#### Scenario: Retention remains independent
- **WHEN** uninstall removes a claim requesting a large retention count
- **THEN** it does not promise immediate content deletion or a reduced effective count while surviving output records still demand that count

### Requirement: Application lifecycle coordinates the claim registry
Install, managed update, rollback, activation, context changes, release, and uninstall SHALL use the separate authoritative claim registry and the same application revision boundary. Applicable app credentials SHALL be checked before preparation or mutation. Successful transitions SHALL publish registry members, policy state, claim selection, application metadata and bindings as one recoverable generation. Failed or stale transitions SHALL NOT recreate removed registries or restore old context/policies. Uninstall SHALL revoke app access and retire its live registry with the live app record while retaining appropriately protected receipts. Existing resource leases SHALL survive registry removal.

#### Scenario: Crash separates member and application publication
- **WHEN** a committed managed update is interrupted between HMAC-protected claim and app publication
- **THEN** recovery completes one coherent claim and binding generation before subsequent access

#### Scenario: Uninstall overtakes ensure
- **WHEN** uninstall commits while an earlier ensure prepares content
- **THEN** ensure cannot republish the application or its claim registry

#### Scenario: Live consumer survives registry removal
- **WHEN** uninstall retires the registry while a native consumer owns a lease
- **THEN** the consumer's output remains protected until its native lifetime ends
