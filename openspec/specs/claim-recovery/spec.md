# claim-recovery Specification

## Purpose

Let applications decide explicitly how to handle unavailable claimed content while distinguishing exact restoration from advancing a saved source request.

## Requirements

### Requirement: Registered reclaim policy controls unavailable claim recovery
The typed policy `reclaim_policy` SHALL accept strategies `report_unavailable`, `restore_exact`, and `refresh`. Its application default and permitted claim overrides SHALL be registered with other claim policies. The default SHALL be `report_unavailable`. Ensure of an unavailable claim and explicit claim recovery SHALL use its effective reclaim policy. Queries, bare paths, and ordinary leases SHALL NOT initiate recovery. Recovery SHALL enforce applicable credentials and source/claim permissions, preserve scope/context/claim identity, and commit selection changes only if the observed application state remains current. Recovery SHALL NOT implicitly activate an executable.

#### Scenario: Default policy reports unavailable content
- **WHEN** ensure finds a reclaimed selected output under `report_unavailable`
- **THEN** it returns `content_unavailable` identifying the authorized claim without network or selection changes

#### Scenario: Concurrent mutation wins recovery
- **WHEN** another claim or policy mutation commits after recovery captured the application revision
- **THEN** recovery reports `revision_conflict` without overwriting the newer state

### Requirement: Exact restoration verifies immutable provenance
`restore_exact` SHALL target the saved immutable source provenance and validate the restored content manifest, digests, and dependency identities before publishing an available replacement. It SHALL NOT substitute a newer moving branch, release, URL body, or local file. Success SHALL select a newly identified output without reviving the reclaimed output UUID. Unobtainable original bytes or provenance SHALL produce `exact_content_unavailable`; content failing recorded integrity SHALL produce `integrity_error`. Neither failure SHALL redirect the claim or trigger a refresh fallback.

#### Scenario: Git branch has advanced
- **WHEN** a claimed Git output is reclaimed and its branch moves before exact recovery
- **THEN** recovery uses the saved commit and verifies its content, or fails without selecting the newer commit

#### Scenario: Local bytes have changed
- **WHEN** the saved local input can no longer supply bytes matching the recorded manifest
- **THEN** exact recovery fails without changing the claim or marking the old UUID ready

### Requirement: Refresh explicitly allows a new resolved version
`refresh` SHALL rerun the saved claim acquisition request under ordinary source cache and refresh rules, validate the candidate, and commit it as the same claim's selection. It SHALL report the resolved output and whether source discovery was refreshed. It SHALL NOT claim to restore the old exact bytes. Explicit low-level `reacquire ENTRY_UUID` SHALL retain its saved-request rerun behavior and SHALL NOT be documented as exact restoration or as moving a claim.

#### Scenario: Refresh selects a newer release
- **WHEN** an unavailable claim uses `refresh` and normal source resolution selects a newer release
- **THEN** the validated release can become that claim's selected output while its active executable binding remains unchanged

#### Scenario: Shared claims recover independently
- **WHEN** two scopes select the same reclaimed output and one successfully refreshes
- **THEN** only the recovering scope's claim moves and the other remains inspectable as unavailable
