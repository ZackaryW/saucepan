# output-policies Specification

## Purpose

Apply explicit item-level output retention and reclaim policies independently of source discovery and ordinary application usage claims.

## Requirements

### Requirement: Acquisition payloads customize output policies
Managed acquisition SHALL select identical-pull, update, revision, reclaim and protection behavior through typed policy UUID references. Effective policies and their definition provenance SHALL be reported and persisted as derived evidence. Revision policy SHALL determine whether acquired history is retained; reclaim SHALL execute that policy. The flat contract SHALL NOT accept inline eviction, revision_policy, policies or claim overrides as an alternative authority. Invalid or conflicting references SHALL fail before acquisition side effects. Output revision selection SHALL NOT change retrieval caching, source refresh, acquisition update frequency or delete source working data. Temporary and caller-owned outputs SHALL reject explicit managed revision/protection choices and SHALL NOT join managed history.

#### Scenario: An app requests a custom retention count
- **WHEN** its acquisition references a latest-three revision policy
- **THEN** the output records that policy reference and derived retention and its source retains its independent discovery/working-data lifecycle

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

### Requirement: Active usage and custom policies coordinate with reclaim
Saucepan SHALL coordinate output use with reclaim so a native resource lease or inherited execution lifetime cannot lose its content while held. Custom `useable_app` protection SHALL override ordinary eviction eligibility for active bindings. Pending validated lifecycle commit intent SHALL also protect its candidate after the preparing process exits, as defined in [application-lifecycle](../application-lifecycle/spec.md). Reclaim SHALL require all applicable protections to end and ordinary retention to permit deletion; ending one protection SHALL NOT override another.

#### Scenario: Eviction runs while an old executable is still running
- **WHEN** a replacement has activated but the previous entry has a live execution lease
- **THEN** reclaim defers deleting that entry until the lease ends and reports the deferred state

### Requirement: Applications register policy defaults and permitted overrides
An application claim registry SHALL reference standalone typed behavior defaults and policy constraints describing permitted values or bounds. Claim acquisition SHALL resolve supplied policy references and validate effective choices before side effects. Nested defaults and override bundles SHALL NOT remain as a second configurable authority. Unknown policy types and choices outside registered constraints SHALL fail with policy_conflict or invalid_request as appropriate. Association changes SHALL require the expected application revision and applicable credentials through the existing administration boundary, and SHALL reject a configuration that invalidates existing claims. Neither default changes nor claim reuse SHALL silently reinterpret surviving claim policies. An explicitly adopted replacement SHALL preserve claim identity, context, selected output and binding while updating policy references and derived evidence. Reclaim policy SHALL govern unavailable-content recovery independently of revision history, source refresh and usable-app protection.

#### Scenario: Allowed retention override
- **WHEN** an application permits retention counts from three through ten and a claim references a latest-five revision policy
- **THEN** the claim records five as its effective retention count and source freshness remains unchanged

#### Scenario: Disallowed override
- **WHEN** a caller requests a policy outside the application's registered allowance
- **THEN** the operation fails before source acquisition, output publication, or claim mutation

#### Scenario: Policy update would invalidate a claim
- **WHEN** a revision-checked policy update disallows a surviving claim's effective policy
- **THEN** it reports `policy_conflict` without changing the registry or claim

### Requirement: No-history policy preserves live consumers
History none SHALL mean no ordinary retained historical output window, not immediate deletion of an in-use revision. New content SHALL receive a new output identity before selection changes. Source cache, surviving policy demand, active bindings, pending intent and leases SHALL retain their independent meanings. Unavailable-content reclaim_policy SHALL remain a recovery choice, not an alias for revision retention.

#### Scenario: Update without history while the old revision is in use
- **WHEN** a history-none acquisition publishes a new revision while a consumer holds the old one
- **THEN** new access selects the new revision and cleanup reports the old revision as deferred
- **AND** later reclaim can remove the old content after all protections and other policy demand permit it
