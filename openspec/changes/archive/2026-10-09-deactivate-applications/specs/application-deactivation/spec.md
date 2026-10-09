# Spec Delta

## Purpose

Allow applications to relinquish their active executable binding without releasing claims or interrupting consumers that already own a resource lease.

## ADDED Requirements

### Requirement: Explicit deactivation clears only the selected binding
Native and CLI callers SHALL be able to deactivate an application by UUID within an explicit store. Deactivation SHALL atomically clear its active binding and report the removed binding, preserving application identity, name, settings, claims, other applications, sources, outputs and installed shims. It SHALL NOT require valid old payload bytes, refresh sources, kill processes or invoke content reclaim.

#### Scenario: Deactivate a bound application
- **WHEN** a caller deactivates an application with an active binding
- **THEN** that binding becomes absent and the result identifies the removed binding
- **AND** claims, settings and installed shims remain unchanged

#### Scenario: Repeat deactivation or select an invalid application
- **WHEN** a caller deactivates an already inactive application
- **THEN** the operation succeeds without rewriting the application and reports no removed binding
- **WHEN** the UUID is missing or identifies an output
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
Deactivation SHALL coordinate with binding selection and application mutations so it preserves concurrently committed claims/settings. Its committed journal intent SHALL recover to the inactive record after interruption. A later activation commit SHALL be allowed to restore a binding; deactivation SHALL NOT create a permanent disabled state.

#### Scenario: Interrupted deactivation is replayed
- **WHEN** a deactivation intent is committed and execution stops before journal cleanup
- **THEN** reopening the store completes the inactive app record without losing its claims or settings
