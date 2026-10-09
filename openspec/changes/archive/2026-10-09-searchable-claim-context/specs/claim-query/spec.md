# Claim query delta

## Purpose

Find an application's existing claims by caller-defined context without changing resource selection, creating claims, or retaining resource lifetimes.

## ADDED Requirements

### Requirement: Queries explicitly select one application's visible claims
A query SHALL require one exact application UUID and a `where` array of zero to 64 conditions. Each condition SHALL contain exactly `key`, `op`, and `value`; `key` and `value` SHALL be strings and `op` SHALL be `eq` or `glob`. Keys SHALL be nonempty, and the key and value byte limits SHALL match claim context. Query objects SHALL reject unknown fields, unknown operators, invalid UUIDs, and invalid conditions. All conditions SHALL be combined with AND. A missing context key SHALL fail a condition, even when its pattern is `*`. Empty `where` SHALL match all claims visible through that application's existing source filters. Conditions SHALL address only literal context keys, not claim fields, UUIDs, or other applications. An unknown, non-application, or removing application SHALL produce an error, not an empty successful result.

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

### Requirement: Context matching uses explicit portable string semantics
Exact equality SHALL compare complete strings case-sensitively without normalization. Glob matching SHALL also match the complete value case-sensitively on every platform. The supported syntax SHALL be `*` for zero or more Unicode scalar values, `?` for exactly one Unicode scalar value, and backslash escaping of `*`, `?`, backslash, brackets, and braces. Unescaped brackets or braces, unsupported escape sequences, and trailing escape characters SHALL be errors. Repeated stars SHALL have the same meaning as one star. Slash SHALL be an ordinary character and wildcards SHALL be able to match it; glob syntax SHALL NOT imply filesystem traversal or recursive directory behavior. There SHALL be no shell expansion, path canonicalization, case folding, or implicit Unicode normalization.

#### Scenario: Match Unicode and separators consistently
- **WHEN** `?` is matched against `é` and `skills-*` against `skills-team/core`
- **THEN** each matches on Windows and Unix
- **AND** `?` does not match the two-scalar decomposed string `e` followed by a combining acute accent

#### Scenario: Distinguish equality from wildcard syntax
- **WHEN** the `eq` value is `skills-*`
- **THEN** it matches only the literal string `skills-*`
- **AND** callers can also escape the star for a literal glob match

#### Scenario: Reject unsupported glob features
- **WHEN** a caller supplies unescaped `[ab]`, `{a,b}`, or a trailing backslash in a glob
- **THEN** the query fails with a condition-specific validation error before producing results

### Requirement: Query results are deterministic metadata snapshots
Queries SHALL return the selected application UUID, its observed revision, and all matching claim views ordered by claim UUID. Each view SHALL include claim identity, request, selected output UUID, context, and current indexed availability. Zero matches SHALL be a successful empty list; multiple matches SHALL all be returned without silently selecting one. Application revision, filters, claim selections, context, and output availability SHALL be read as one coordinated metadata snapshot. A later mutation SHALL NOT alter an already returned result. Corrupt or missing output metadata referenced by a matching claim SHALL fail the query rather than silently omit that claim.

#### Scenario: Multiple matching claims remain distinct
- **WHEN** two claims have identical matching context
- **THEN** both appear once in UUID order with their own selected outputs

#### Scenario: Query races an update
- **WHEN** context or claim selection changes while a query reads metadata
- **THEN** the query returns either the coherent pre-change or post-change snapshot and its corresponding application revision

#### Scenario: Reclaimed output remains searchable
- **WHEN** a matching claim selects a reclaimed output whose provenance remains present
- **THEN** its view remains in the result with reclaimed availability

### Requirement: Search does not mutate selections or grant a resource lifetime
The native and CLI query interfaces SHALL implement the same contract. The CLI SHALL accept `query-claims FILE` with an explicit store root and a JSON object containing `app` and `where`, and SHALL emit one JSON result on stdout with exit zero for valid queries, including zero matches. Invalid input or store errors SHALL exit nonzero without a successful result. Queries SHALL NOT acquire, refresh, probe, hash payloads, create claims, update application revisions, or retain leases. Existing store-open transaction recovery SHALL remain permitted. Query results SHALL NOT be authentication grants. Mutation and leased access SHALL continue requiring concrete UUIDs; searching followed by access SHALL NOT promise an atomic search-and-lease operation.

#### Scenario: Search remains independent of network and payload access
- **WHEN** a valid query runs while a source is unreachable or its discovery cache is expired
- **THEN** it succeeds using indexed metadata without refreshing the source or reading content files

#### Scenario: Output changes after searching
- **WHEN** a claim changes after a query returns
- **THEN** exact-output leasing addresses the returned output UUID while claim-based leasing selects the claim's then-current output
- **AND** either access operation still performs its existing availability and lease checks
