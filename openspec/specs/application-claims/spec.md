# application-claims Specification

## Purpose

Track which exact acquired outputs an application uses while retaining the acquisition request needed for independent updates and reacquisition.

## Requirements

### Requirement: Applications claim specific outputs and retain their requests
Registered applications SHALL have UUID-indexed records with settings and claims. Each claim SHALL retain a claim identity, the acquisition request including item policies, its selected output UUID, and a plain string-to-string context map. Omitted context SHALL mean an empty map in the current contract. Successful scoped acquisition SHALL create or update the intended claim; failed acquisition SHALL leave existing claims unchanged. Supplied acquisition context SHALL be published atomically with a new claim. Claims SHALL represent usage associations rather than exclusive resource ownership or authentication proof. Context SHALL NOT participate in source identity, output identity, item grouping, deduplication, or retention policy.

#### Scenario: An application claims two selections from one repository
- **WHEN** it acquires two different folders or platform selections
- **THEN** distinct claims preserve the corresponding requests and selected entries without collapsing them into a source-only claim

#### Scenario: Two project contexts share content
- **WHEN** an application acquires identical content with reuse policy under two different project context values
- **THEN** each acquisition creates its own claim carrying the supplied context while both can select the same output
- **AND** neither context creates a project directory in the store

#### Scenario: Acquisition fails with supplied context
- **WHEN** a context-bearing acquisition fails before claim publication
- **THEN** no partial claim or context-only association is published

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

### Requirement: Claim context is explicit application-owned metadata
Context keys SHALL be nonempty strings and values SHALL be strings, including empty strings. Keys and values SHALL be case-sensitive and preserved without path, whitespace, or Unicode normalization. Dots in keys SHALL be literal characters, not nested field paths. Context SHALL permit at most 64 entries, keys of at most 128 UTF-8 bytes, and values of at most 4096 UTF-8 bytes. Invalid context SHALL fail before source acquisition or claim mutation. Identical context on multiple claims SHALL be permitted and SHALL NOT imply uniqueness, ownership, or automatic future acquisition.

#### Scenario: Context remains plain data
- **WHEN** a caller supplies a nested object or number as a context value, an empty key, or an over-limit map
- **THEN** the operation fails without acquiring content or changing claims

#### Scenario: Literal values are preserved
- **WHEN** context contains a Windows path, a dotted key, or a value containing `*`
- **THEN** the stored context preserves those strings literally and does not interpret them as queries

### Requirement: Existing claim context can be replaced explicitly
The native and CLI interfaces SHALL support whole-map context replacement using explicit application and claim UUIDs plus the caller's expected application revision. A successful change SHALL preserve claim identity, request, output selection, active binding, and other claims, and SHALL increment the application revision once. An identical replacement at the expected revision SHALL be a no-op. An empty map SHALL clear context. Missing claims, removed/removing applications, invalid input, and revision mismatches SHALL fail without modifying application state; the revision check SHALL also precede no-op success. Replacement SHALL use the existing recoverable metadata transaction mechanism and SHALL NOT acquire content or refresh sources.

#### Scenario: Clear context without moving a claim
- **WHEN** a caller replaces a claim's context with an empty map at the current application revision
- **THEN** that claim has empty context and still selects its original output

#### Scenario: Reject stale context replacement
- **WHEN** another application mutation commits after the caller reads the expected revision
- **THEN** context replacement reports a conflict without overwriting newer state

### Requirement: Claim transitions preserve context
Primitive claim updates, managed updates, rollback, deactivation, and recovery SHALL preserve the context of surviving claims. A primitive update SHALL preserve context present at its commit boundary, including a concurrent completed context replacement. A managed transition prepared against an earlier application revision SHALL report the existing lifecycle conflict rather than restore stale context. Scoped acquisition results SHALL reflect the claim actually committed. Managed installation and bootstrap SHALL create empty context unless context is subsequently explicitly replaced. Releasing a claim or uninstalling its application SHALL remove its context with that claim. Reclaiming content SHALL leave surviving claim context inspectable.

#### Scenario: Context changes during primitive acquisition
- **WHEN** context replacement commits while an existing claim's primitive update acquires a candidate
- **THEN** a successful claim update retains the newly committed context and returns that context

#### Scenario: Context changes during managed preparation
- **WHEN** context replacement commits after managed update or rollback captures the application revision
- **THEN** the managed operation conflicts without restoring the old context or changing the active binding

#### Scenario: Recovery and unavailable content preserve metadata
- **WHEN** a context transaction is recovered and the selected output is later reclaimed
- **THEN** the surviving claim exposes its committed context and unavailable content state
