# Proposal

## Why

Saucepan's individual lifecycle operations work, but their combined failure and recovery behavior is incomplete: self-update moves a claim before probing, bootstrap can leave partial application state, and shims have no ownership-backed maintenance or uninstall workflow. One coordinated change must define and verify the entire application lifecycle before adding more isolated commands.

## What Changes

- Define one lifecycle contract spanning generic installation, Saucepan bootstrap/self-update, managed update, rollback, existing deactivation, uninstall, shim maintenance, claims, leases, and retention.
- Prepare and probe managed candidates before committing the selected claim and active binding together. Preserve independent resource-only claim updates and explicit binding-only activation/deactivation.
- Add persistent operation identity, inspectable progress and resumable recovery for multi-file installation, shim maintenance and removal. Interrupted or busy operations report pending work without blocking unrelated applications.
- Record exact shim ownership and verify it before removing or replacing files. Handle executable/sidecar pairs, destination aliases, occupied names, damaged files, and Windows running-launcher constraints.
- Add uninstall that removes the live application registration and its claims/binding, cleans up owned shims recoverably, and preserves sources, shared outputs, provenance and existing consumers. Later reclaim follows the existing retention rules.
- Reconcile the completed changes in a behavior matrix and verify one integrated sequence, including faults and concurrent operations, with real builds in isolated simulated roots.

## Capabilities

### New Capabilities

- `application-lifecycle`: coordinated installation, managed update/rollback, deactivation, removal, operation reporting and concurrency semantics.
- `shim-maintenance`: exclusive launcher ownership, recoverable publication/replacement/removal, collision checks and explicit treatment of existing unregistered shims.
- `lifecycle-recovery`: durable operation intent, idempotent retry, reconciliation, fault recovery and integrated acceptance.

### Modified Capabilities

None in the main spec tree: `openspec list --specs --json` reports no published specs. This proposal builds on completed, unarchived `reimplement-saucepan`, `efficient-resource-access`, `clean-lease-markers`, and `deactivate-applications`. The design explicitly reconciles their contracts; those artifacts are neither silently edited nor archived. When publishing specs, these deltas must be reconciled together, especially the stronger self-update claim/binding guarantee.

## Impact

Primary areas are `src/models/index`, `src/core/{index,activation,bootstrap,launch,store}`, new focused lifecycle/shim modules, the CLI, tests and acceptance documentation. Add lifecycle metadata without introducing a schema-version hierarchy or legacy migration. Existing application/output records and sidecars remain readable; shim ownership requires explicit registration/adoption before destructive maintenance. Older executables are not permitted to write concurrently with the new lifecycle protocol.

No new external dependency is planned. The initial verified host remains Windows; lifecycle code keeps portable paths and native lease semantics, without claiming unexecuted OS coverage.

## Non-goals

Release packaging/distribution and a full platform/MSRV verification project are excluded by the user's scope choice. Also excluded: new providers, SDK ports, encrypted/authenticated views, legacy schema compatibility, dependency solving, arbitrary installer scripts, source deletion, forced shared-output purge, automatic background maintenance, metadata compaction, or changing the existing mock installation during this proposal.
