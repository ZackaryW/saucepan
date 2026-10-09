# Proposal

## Why

Releasing claims does not clear an application's active binding. Users need an explicit way to stop future launches and remove that binding's retention protection while allowing existing consumers to finish.

## What Changes

- Add native and CLI deactivation that atomically clears only the selected app's active binding.
- Preserve claims, settings, installed shims, source/output records and running leases.
- Make repeated deactivation a successful no-op; report the removed binding when present.
- Allow ordinary reclaim to evaluate the former output on subsequent operations, respecting other apps, retention and live consumers.

## Capabilities

### New Capabilities

- `application-deactivation`: explicit, idempotent binding removal independent of claims and resource lifetime.

### Modified Capabilities

None published: the main spec tree remains empty. This extends the implemented, unarchived self-management and resource-access baseline.

## Impact

Core activation, CLI command routing, managed/recovery tests and documentation. No schema change, uninstall, process termination, immediate deletion, shim replacement or production/mock mutation.
