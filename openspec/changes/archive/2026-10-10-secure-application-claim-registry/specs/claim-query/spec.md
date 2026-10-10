# Spec Delta

## MODIFIED Requirements

### Requirement: Queries explicitly select one application's visible claims
A query SHALL require one exact application UUID and a `where` array of zero to 64 conditions. An optional `scope` SHALL restrict candidates to one literal explicit scope; omission SHALL include all visible scopes in that app. Scope SHALL obey the claim scope constraints. The application's configured credential requirement and source/claim policy SHALL be checked before restricted candidate access, including empty results. Each condition SHALL contain exactly `key`, `op`, and `value`; `key` and `value` SHALL be strings and `op` SHALL be `eq` or `glob`. Keys SHALL be nonempty, and the key and value byte limits SHALL match claim context. Query objects SHALL reject unknown fields, unknown operators, invalid UUIDs, and invalid conditions. All conditions SHALL be combined with AND. A missing context key SHALL fail a condition, even when its pattern is `*`. Empty `where` SHALL match claims permitted by that application's access policies, optional exact scope, and existing source filters. Conditions SHALL address only literal context keys, not claim fields, UUIDs, or other applications. An unknown, non-application, or removing application SHALL produce an error, not an empty successful result.

#### Scenario: Find project-specific aliases
- **WHEN** the query requires `project eq C:/work/project-a` and `source_alias glob skills-*`
- **THEN** it returns only visible claims in the selected application satisfying both conditions
- **AND** identical context in another application does not appear

#### Scenario: Respect missing keys and view filters
- **WHEN** an application has claims missing the queried key and claims excluded by its source filter
- **THEN** neither group appears in the matching results
- **AND** an empty condition list still respects that source filter

#### Scenario: Validate even when there are no candidates
- **WHEN** a query includes an invalid operator or malformed pattern but the application has no matching claims
- **THEN** the query fails validation instead of returning an empty successful result

### Requirement: Query results are deterministic metadata snapshots
Queries SHALL return the selected application UUID, its observed revision, and all matching claim views ordered by claim UUID. Each view SHALL include claim identity, request, selected output UUID, explicit scope, context, and current indexed availability. A covered request view SHALL NOT appear as a second stored claim. Zero matches SHALL be a successful empty list; multiple matches SHALL all be returned without silently selecting one. Application revision, filters, claim selections, context, and output availability SHALL be read from the authenticated registry and index as one coordinated metadata snapshot. Missing or corrupt required registry/member data SHALL fail rather than silently omit claims. A later mutation SHALL NOT alter an already returned result. Corrupt or missing output metadata referenced by a matching claim SHALL fail the query rather than silently omit that claim.

#### Scenario: Multiple matching claims remain distinct
- **WHEN** two claims have identical matching context
- **THEN** both appear once in UUID order with their own selected outputs

#### Scenario: Query races an update
- **WHEN** context or claim selection changes while a query reads metadata
- **THEN** the query returns either the coherent pre-change or post-change snapshot and its corresponding application revision

#### Scenario: Reclaimed output remains searchable
- **WHEN** a matching claim selects a reclaimed output whose provenance remains present
- **THEN** its view remains in the result with reclaimed availability

## ADDED Requirements

### Requirement: Generic discovery includes only explicitly public scopes
A separate generic discovery operation SHALL search only claims in explicitly public registered scopes that also satisfy applicable source/claim discovery permissions. Absence of public exposure SHALL exclude that scope, even with credentials disabled. Anonymous generic lookup SHALL omit candidates whose configured proof requirement cannot be satisfied. The existing explicit-app query SHALL retain its app boundary and SHALL NOT become an unrestricted fallback. Generic conditions SHALL use the same context validation and matching semantics as app queries. Results SHALL identify their owning apps, claims and scopes, be ordered by app then claim UUID, and report per-app revisions from a coordinated snapshot. Discovery SHALL NOT create a claim, acquire content, grant mutation rights, or retain a resource lease.

#### Scenario: App has one non-public scope
- **WHEN** an app is registered for `project-a` with no public scope and credentials disabled
- **THEN** generic discovery excludes its claims while explicit app operations remain subject to registered permissions

#### Scenario: Public discovery does not grant claiming rights
- **WHEN** generic discovery finds a public claim but the requesting app is forbidden to acquire its source
- **THEN** the subsequent app acquisition fails with `access_denied`
