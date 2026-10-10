# application-deactivation Specification

## Purpose

Allow applications to relinquish their active executable binding without releasing claims or interrupting consumers that already own a resource lease.

## Requirements

### Requirement: Explicit deactivation clears only the selected binding
Native and CLI callers SHALL be able to deactivate a ready application by UUID within the selected store, subject to the application access contract in [claim-security](../claim-security/spec.md). CLI root selection SHALL follow [store-layout](../store-layout/spec.md). Deactivation SHALL atomically clear its active binding and report the removed binding, preserving application identity, name, settings, claims, other applications, sources, outputs and installed shims. An actual binding change SHALL advance the application's mutation revision. It SHALL NOT require valid old payload bytes, refresh sources, kill processes or invoke content reclaim.

#### Scenario: Deactivate a bound application
- **WHEN** a caller deactivates an application with an active binding
- **THEN** that binding becomes absent and the result identifies the removed binding
- **AND** claims, settings and installed shims remain unchanged

#### Scenario: Repeat deactivation or select an invalid application
- **WHEN** a caller deactivates an already inactive application
- **THEN** the operation succeeds without rewriting the application and reports no removed binding
- **WHEN** the UUID is missing, identifies an output, or identifies a removing application
- **THEN** the operation fails without writing a binding change

### Requirement: Deactivation preserves resource lifetime and normal retention
A launch that selected and leased a binding before deactivation SHALL remain protected until its native lifetime ends. A later launch with no intervening activation SHALL fail because there is no active binding. Subsequent reclaim SHALL respect other applications' active bindings, ordinary retention and existing leases. Reactivation SHALL remain available through normal activation validation.

#### Scenario: Deactivate while a target is running
- **WHEN** an app is deactivated after its target has started and the output is outside retention
- **THEN** future shim launches fail while the original target continues and reclaim defers its content
- **AND** after all live protection ends, a later reclaim can remove the output

#### Scenario: Another application or retention still protects the output
- **WHEN** one app deactivates while another app still actively binds the same protected output, or the output is within retention
- **THEN** reclaim preserves that output according to the remaining protection

### Requirement: Deactivation uses recoverable serialized binding mutation
Deactivation SHALL coordinate with binding selection and application mutations so it preserves concurrently committed claims/settings. Its committed journal intent SHALL recover to the inactive record after interruption. A later binding-only activation commit SHALL be allowed to restore a ready application's binding; deactivation SHALL NOT create a permanent disabled state. Managed updates SHALL instead reject intervening mutation, and uninstall SHALL fence both activation forms, as defined in [application-lifecycle](../application-lifecycle/spec.md).

#### Scenario: Interrupted deactivation is replayed
- **WHEN** a deactivation intent is committed and execution stops before journal cleanup
- **THEN** reopening the store completes the inactive app record without losing its claims or settings
