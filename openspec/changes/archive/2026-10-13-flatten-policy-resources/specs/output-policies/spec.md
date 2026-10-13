## MODIFIED Requirements

### Requirement: Acquisition payloads customize output policies
Managed acquisition SHALL select identical-pull, update, revision, reclaim and protection behavior through typed policy UUID references. Effective policies and their definition provenance SHALL be reported and persisted as derived evidence. Revision policy SHALL determine whether acquired history is retained; reclaim SHALL execute that policy. The flat contract SHALL NOT accept inline eviction, revision_policy, policies or claim overrides as an alternative authority. Invalid or conflicting references SHALL fail before acquisition side effects. Output revision selection SHALL NOT change retrieval caching, source refresh, acquisition update frequency or delete source working data. Temporary and caller-owned outputs SHALL reject explicit managed revision/protection choices and SHALL NOT join managed history.

#### Scenario: An app requests a custom retention count
- **WHEN** its acquisition references a latest-three revision policy
- **THEN** the output records that policy reference and derived retention and its source retains its independent discovery/working-data lifecycle

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
