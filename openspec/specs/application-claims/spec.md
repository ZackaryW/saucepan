# application-claims Specification

## Purpose

Track which exact acquired outputs an application uses while retaining the acquisition request needed for independent updates and reacquisition.

## Requirements

### Requirement: Applications claim specific outputs and retain their requests
Registered applications SHALL have UUID-indexed records with settings and claims. Each claim SHALL retain a claim identity, the acquisition request including item policies, and its selected output UUID. Successful scoped acquisition SHALL create or update the intended claim; failed acquisition SHALL leave existing claims unchanged. Claims SHALL represent usage associations rather than exclusive resource ownership or authentication proof.

#### Scenario: An application claims two selections from one repository
- **WHEN** it acquires two different folders or platform selections
- **THEN** distinct claims preserve the corresponding requests and selected entries without collapsing them into a source-only claim

### Requirement: Application updates do not move other applications' claims
Primitive `update-claim` SHALL acquire a valid replacement before selecting its entry and SHALL NOT change the active binding. Managed update and rollback SHALL instead follow the coordinated claim/binding contract in [application-lifecycle](../application-lifecycle/spec.md). Other claims SHALL continue referencing their original entries until explicitly updated. Releasing a claim SHALL remove only that association, not implicitly delete shared source data or another app's claim.

#### Scenario: Two applications update independently
- **WHEN** apps A and B claim entry E1 and B successfully updates to E2
- **THEN** B selects E2 and A still selects E1

### Requirement: Reclaimed outputs remain explainable through claims
Ordinary claims SHALL NOT prevent policy-driven reclaim. App views SHALL expose claimed entries' availability, and path lookup SHALL report reclaimed or pending-reclaim entries as unavailable. `reacquire ENTRY_UUID` SHALL use the output's saved request to return a ready entry without moving an application claim. `update-claim` SHALL reacquire using the claim's saved request and move that claim on success. Neither operation SHALL revive the reclaimed entry's identity or imply activation.

#### Scenario: An application's older output is reclaimed
- **WHEN** eviction reclaims an entry referenced by an ordinary claim
- **THEN** the claim and request remain visible with unavailable content status
- **AND** an explicit reacquisition can establish a new ready selection

### Requirement: App views are scoped to claims
An app view SHALL contain its claims and their referenced metadata, subject to its configured view filters. A matching source cached only by another app SHALL NOT appear as a claim. Listing SHALL NOT create claims, and view filters SHALL NOT act as acquisition permissions.

#### Scenario: Another application caches a matching resource
- **WHEN** app A lists its view after B acquires an otherwise matching item
- **THEN** A has no claim to that item unless A actually acquires or explicitly claims it
