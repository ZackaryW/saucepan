## MODIFIED Requirements

### Requirement: Query results are deterministic metadata snapshots
Queries SHALL return the selected application UUID, its observed revision, and all matching claim views ordered by claim UUID. Each view SHALL include claim identity, request, selected output UUID, explicit scope, context, and current indexed availability. A covered request view SHALL NOT appear as a second stored claim. Zero matches SHALL be a successful empty list; multiple matches SHALL all be returned without silently selecting one. Application revision, filters, claim selections, context, and output availability SHALL be read from the registry and index as one coordinated metadata snapshot verified according to the store's selected integrity mode. Missing or corrupt required registry/member data SHALL fail rather than silently omit claims. A later mutation SHALL NOT alter an already returned result. Corrupt or missing output metadata referenced by a matching claim SHALL fail the query rather than silently omit that claim.

#### Scenario: Multiple matching claims remain distinct
- **WHEN** two claims have identical matching context
- **THEN** both appear once in UUID order with their own selected outputs

#### Scenario: Query races an update
- **WHEN** context or claim selection changes while a query reads metadata
- **THEN** the query returns either the coherent pre-change or post-change snapshot and its corresponding application revision

#### Scenario: Reclaimed output remains searchable
- **WHEN** a matching claim selects a reclaimed output whose provenance remains present
- **THEN** its view remains in the result with reclaimed availability
