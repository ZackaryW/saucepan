## Purpose

Apply explicit item-level output retention and reclaim policies independently of source discovery and ordinary application usage claims.

## ADDED Requirements

### Requirement: Acquisition payloads customize output policies
An acquisition payload SHALL accept identical-pull and eviction policies for its item. Effective policies SHALL be persisted and reported. Invalid policies SHALL fail before acquisition side effects. Output eviction overrides SHALL NOT change source refresh settings or delete source working data.

#### Scenario: An app requests a custom retention count
- **WHEN** its acquisition sets latest-entry retention to three
- **THEN** the output records that override and its source retains its independent discovery/working-data lifecycle

### Requirement: Default retention keeps the latest ten entries per requested item
Absent an override, Saucepan SHALL retain the latest ten output entries for a logical item defined by source, selector, and normalized selection, excluding resolved version and lifecycle policies. Ordering SHALL use committed creation order, not reads or semantic version sorting. Compatible reuse SHALL NOT consume a new slot. When attached policies demand different positive counts within an item scope, the effective window SHALL be their maximum. Protected or in-use entries SHALL be reported as deferred reclaim exceptions.

#### Scenario: An eleventh ordinary entry is committed
- **WHEN** an item with default policy publishes its eleventh entry and no entries are protected or in use
- **THEN** the oldest entry's content is reclaimed and the latest ten remain available
- **AND** another platform/folder item's history is unaffected

#### Scenario: Different retention requests coexist
- **WHEN** attached policies in one item scope request counts of three and ten
- **THEN** the effective window is ten and that result is reported without overwriting either policy

### Requirement: Reclaim removes output content without erasing provenance
Eviction SHALL reclaim eligible stored content even when ordinary applications claim it. It SHALL retain index provenance and request information, leave sources unchanged, and distinguish reclaim_pending from fully reclaimed. Failed deletion SHALL be retriable and SHALL NOT be reported as successful removal.

#### Scenario: Deletion fails because a file is unavailable for removal
- **WHEN** reclaim cannot finish deleting an output
- **THEN** the entry remains reclaim_pending, is unavailable for new acquisitions as a reusable entry, and can be retried

### Requirement: Active usage and custom policies coordinate with reclaim
Saucepan SHALL coordinate output use with reclaim so an acquired execution lease cannot lose its content mid-launch or execution. Custom `useable_app` protection SHALL override ordinary eviction eligibility for active bindings. After protection ends, normal retention SHALL become effective again.

#### Scenario: Eviction runs while an old executable is still running
- **WHEN** a replacement has activated but the previous entry has a live execution lease
- **THEN** reclaim defers deleting that entry until the lease ends and reports the deferred state
