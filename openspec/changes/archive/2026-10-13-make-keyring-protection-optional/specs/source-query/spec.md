## RENAMED Requirements

- FROM: `### Requirement: Source visibility is explicit authenticated administration`
- TO: `### Requirement: Source visibility is explicit verified administration`
- FROM: `### Requirement: Source query results are authenticated metadata snapshots`
- TO: `### Requirement: Source query results are verified metadata snapshots`

## MODIFIED Requirements

### Requirement: Source visibility is explicit verified administration
Trusted local administration SHALL be able to set an existing source's public-discovery flag to true or false through native and CLI interfaces. The update SHALL preserve source identity, provider definition and discovery cache and protect the changed metadata using the store-selected integrity mode. Registration retries SHALL preserve the configured visibility. App sessions SHALL not configure visibility. Existing supported source records missing the flag SHALL read as private; unsigned legacy stores SHALL remain unsupported. Explicitly initialized basic stores SHALL be supported without keyring access and SHALL retain the same private-by-default visibility.

#### Scenario: Publish and withdraw
- **WHEN** an administrator publishes and then withdraws a source
- **THEN** subsequent anonymous queries include and then omit it without changing its UUID or claims

### Requirement: Source query results are verified metadata snapshots
Queries SHALL return the observed catalogue generation and compact source views containing UUID, source definition and public-discovery flag, ordered by UUID. They SHALL read a coordinated snapshot verified according to the store's selected integrity mode and fail on corrupt or missing catalogue source records rather than silently skipping them. They SHALL NOT access provider payloads, fetch releases, refresh caches, create claims, retain leases or mutate metadata; ordinary store-open recovery remains permitted. Native queries and `query-sources FILE` SHALL share this contract. CLI queries without `--as-app` SHALL use anonymous visibility; queries with it SHALL use the app session. Query errors SHALL use the existing structured error mechanism, exit nonzero and emit no success stdout.

#### Scenario: Offline metadata search
- **WHEN** source payloads are missing or providers are unreachable
- **THEN** queries still return registered metadata without network requests or metadata changes

#### Scenario: Tampered source
- **WHEN** a catalogue source record has invalid integrity verification or is missing
- **THEN** source querying fails with an integrity error
