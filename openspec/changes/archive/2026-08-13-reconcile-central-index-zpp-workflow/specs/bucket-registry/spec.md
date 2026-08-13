## Purpose

Define the public lifecycle of registered bucket and central-index targets, including local persistence, uniqueness, pinning, and explicit refresh.

## ADDED Requirements

### Requirement: Bucket registry is inspectable
Saucepan SHALL list registered bucket targets in registration order and SHALL report clearly when none are registered.

#### Scenario: Empty bucket registry
- **WHEN** a user runs `bucket list` with no registered buckets
- **THEN** Saucepan succeeds and reports that no buckets are registered

### Requirement: Registration is unique and persistent
`bucket add <target>` SHALL persist a previously unregistered target without fetching it, and SHALL reject a duplicate target without changing the registry.

#### Scenario: Add an unpinned target
- **WHEN** a user adds an unregistered bucket target without `--ref`
- **THEN** Saucepan records it locally, reports success, and performs no network fetch

#### Scenario: Reject duplicate registration
- **WHEN** a user adds a target already present in the registry
- **THEN** Saucepan fails with an already-registered error and preserves the registry

### Requirement: Registration may resolve a pin atomically
`bucket add <target> --ref <ref>` SHALL fetch and resolve the requested ref immediately, persist its resolved commit on success, and roll back registration if resolution fails.

#### Scenario: Add a pinned index
- **WHEN** a user adds a repository-target index with a resolvable `--ref`
- **THEN** Saucepan records the target, requested ref, and resolved commit

#### Scenario: Pinned registration fails
- **WHEN** the requested ref cannot be resolved
- **THEN** Saucepan fails and leaves no registration for the target

### Requirement: Registered targets can be refreshed or removed
`bucket refresh <target>` SHALL fetch an existing registration and update its resolved commit, while `bucket remove <target>` SHALL remove only an existing registration.

#### Scenario: Refresh a registered target
- **WHEN** a user refreshes a registered bucket target whose fetched state has advanced
- **THEN** Saucepan records the newly resolved commit and reports success

#### Scenario: Remove a registered target
- **WHEN** a user removes a registered bucket target
- **THEN** Saucepan deletes that registration and reports success

#### Scenario: Unknown target
- **WHEN** a user refreshes or removes a target that is not registered
- **THEN** Saucepan returns not found and leaves the registry unchanged
