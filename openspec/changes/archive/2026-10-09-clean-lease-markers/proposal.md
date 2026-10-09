# Proposal

## Why

Five bare path lookups currently leave five closed lifetime marker files, even after reclaim while the output remains retained. Repeated reads should not grow bookkeeping, and dead process markers should be cleaned independently of output retention.

## What Changes

- Resolve metadata-only addresses without constructing a lifetime guard.
- Use existing shared entry coordination for native leases and full scans; allocate inheritable lifetime markers only at dedicated process launch.
- Sweep closed recognized lifetime markers during reclaim, including markers for retained outputs, without removing live or inherited ownership.
- Retain the existing native marker format for already-installed launchers.

## Capabilities

### New Capabilities

- `lease-marker-cleanup`: marker-free non-process access and native-evidence cleanup independent of content retention.

### Modified Capabilities

None published: the main spec tree is empty. This adds cleanup requirements to the implemented, unarchived resource-access baseline.

## Impact

Core store access, reclaim, the private launch boundary, focused tests, and resource-access documentation. No request/index schema, retention policy, dependencies, daemon, application lifecycle, or production/mock installation changes.
