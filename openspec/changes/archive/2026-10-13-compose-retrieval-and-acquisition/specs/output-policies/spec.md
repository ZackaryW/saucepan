## RENAMED Requirements

- FROM: `### Requirement: Default retention keeps the latest ten entries per requested item`
- TO: `### Requirement: Revision policies govern retained output history`

## MODIFIED Requirements

### Requirement: Acquisition payloads customize output policies
Managed acquisition SHALL accept identical-pull and revision policies for its copied item. Effective policies SHALL be persisted and reported. A revision policy SHALL determine whether acquired history is retained; reclaim SHALL execute that policy. New composable requests SHALL use revision_policy; current convenience eviction settings SHALL map to that same policy without a second retention evaluator. Mixed or conflicting policy spellings SHALL fail. Invalid policies SHALL fail before acquisition side effects. Output revision overrides SHALL NOT change retrieval caching, source refresh, acquisition update frequency or delete source working data. Temporary and caller-owned outputs SHALL reject explicit managed revision policies and SHALL not join managed history.

#### Scenario: An app requests a custom retention count
- **WHEN** its acquisition sets latest-entry retention to three
- **THEN** the output records that override and its source retains its independent discovery/working-data lifecycle

### Requirement: Revision policies govern retained output history
Managed revision policy SHALL support history none, which retains only the newest revision, and history latest with a positive keep count including the newest. Absent an override, Saucepan SHALL retain the latest ten output entries for a logical item defined by source, selector, and normalized selection, excluding resolved version and lifecycle policies. Ordering SHALL use committed creation order, not reads or semantic version sorting. Compatible reuse SHALL NOT consume a new slot. The effective window SHALL be the maximum positive retention count demanded by non-reclaimed output records and current application claims in that item scope. Removing a claim SHALL NOT erase demand still carried by an output. Protected or in-use entries SHALL be reported as deferred reclaim exceptions even under history none; old bytes SHALL not be overwritten to avoid those exceptions. Unchanged-content reuse during an update SHALL not consume a revision slot. Independent scopes sharing an item SHALL retain the same maximum surviving demand rule.

#### Scenario: An eleventh ordinary entry is committed
- **WHEN** an item with default policy publishes its eleventh entry and no entries are protected or in use
- **THEN** the oldest entry's content is reclaimed and the latest ten remain available
- **AND** another platform/folder item's history is unaffected

#### Scenario: Different retention requests coexist
- **WHEN** attached policies in one item scope request counts of three and ten
- **THEN** the effective window is ten and that result is reported without overwriting either policy

### Requirement: Reclaim removes output content without erasing provenance
Reclaim under revision policy SHALL remove eligible stored content even when ordinary applications claim it. It SHALL retain index provenance and request information, leave sources unchanged, and distinguish reclaim_pending from fully reclaimed. Failed deletion SHALL be retriable and SHALL NOT be reported as successful removal.

#### Scenario: Deletion fails because a file is unavailable for removal
- **WHEN** reclaim cannot finish deleting an output
- **THEN** the entry remains reclaim_pending, is unavailable for new acquisitions as a reusable entry, and can be retried

## ADDED Requirements

### Requirement: No-history policy preserves live consumers
History none SHALL mean no ordinary retained historical output window, not immediate deletion of an in-use revision. New content SHALL receive a new output identity before selection changes. Source cache, surviving policy demand, active bindings, pending intent and leases SHALL retain their independent meanings. Unavailable-content reclaim_policy SHALL remain a recovery choice, not an alias for revision retention.

#### Scenario: Update without history while the old revision is in use
- **WHEN** a history-none acquisition publishes a new revision while a consumer holds the old one
- **THEN** new access selects the new revision and cleanup reports the old revision as deferred
- **AND** later reclaim can remove the old content after all protections and other policy demand permit it
