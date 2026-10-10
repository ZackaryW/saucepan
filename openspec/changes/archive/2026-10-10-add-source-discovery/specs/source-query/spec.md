# Source Query Delta

## Purpose

Find registered sources before acquiring content, with independently controlled public discovery and application permissions.

## ADDED Requirements

### Requirement: Source queries are independent of claims and scopes
Queries SHALL accept optional exact `uuid`, `type`, `repo`, `origin` and `url` fields combined with AND. Supported stored types SHALL be `local`, `url`, `git` and `github_release`; registered `github_repo` inputs normalize to `git`. `repo` SHALL match GitHub release repository strings, `origin` Git origins and `url` URL sources, case-sensitively against stored values without fetching or canonicalizing inputs. Missing applicable fields SHALL not match. Empty objects SHALL match all permitted sources including unclaimed sources. Unknown fields, invalid UUIDs or types, and empty or over-4096-byte string filters SHALL fail validation. Claim scope SHALL not be a source query field.

#### Scenario: Discover before acquiring
- **WHEN** an application searches a registered release repository without any claims or downloaded outputs
- **THEN** the matching source is returned with its UUID and source definition

#### Scenario: Strict filters
- **WHEN** a query supplies an unknown field, a scope, an invalid type, or an invalid filter string
- **THEN** it fails even when the catalogue is empty

### Requirement: Application and anonymous source discovery use distinct visibility rules
Application source queries SHALL verify optional credentials according to the registration and require the discover action. They SHALL intersect registered source allowances and the application's existing source view filter. Omitted source allowances SHALL be unrestricted and empty allowances SHALL yield no sources. Claim scope allowances SHALL not restrict source metadata queries. Anonymous queries SHALL expose only sources explicitly configured for public discovery. Source public visibility SHALL default to false and SHALL be independent of public claims and credentials on other applications. Discovering a source SHALL not authorize its acquisition, claim access or output leasing.

#### Scenario: Application permission boundaries
- **WHEN** an application with a restricted source allowance and no allowed claim scopes searches all sources
- **THEN** it receives only the allowed sources without needing a claim scope
- **AND** subsequent acquisition still requires an allowed scope and acquire action

#### Scenario: Credentials and denied action
- **WHEN** required credentials are missing or invalid, or discover is not allowed
- **THEN** the query fails without a successful result even if no sources exist

#### Scenario: Public claims do not publish sources
- **WHEN** a source has public claims but its source visibility remains private
- **THEN** anonymous source queries omit it
- **AND** explicitly publishing the source does not publish its private claims

### Requirement: Source visibility is explicit authenticated administration
Trusted local administration SHALL be able to set an existing source's public-discovery flag to true or false through native and CLI interfaces. The update SHALL preserve source identity, provider definition and discovery cache and authenticate the changed metadata. Registration retries SHALL preserve the configured visibility. App sessions SHALL not configure visibility. Existing authenticated source records missing the flag SHALL read as private; unsigned legacy stores SHALL remain unsupported.

#### Scenario: Publish and withdraw
- **WHEN** an administrator publishes and then withdraws a source
- **THEN** subsequent anonymous queries include and then omit it without changing its UUID or claims

### Requirement: Source query results are authenticated metadata snapshots
Queries SHALL return the observed catalogue generation and compact source views containing UUID, source definition and public-discovery flag, ordered by UUID. They SHALL read a coordinated authenticated snapshot and fail on corrupt or missing catalogue source records rather than silently skipping them. They SHALL NOT access provider payloads, fetch releases, refresh caches, create claims, retain leases or mutate metadata; ordinary store-open recovery remains permitted. Native queries and `query-sources FILE` SHALL share this contract. CLI queries without `--as-app` SHALL use anonymous visibility; queries with it SHALL use the app session. Query errors SHALL use the existing structured error mechanism, exit nonzero and emit no success stdout.

#### Scenario: Offline metadata search
- **WHEN** source payloads are missing or providers are unreachable
- **THEN** queries still return registered metadata without network requests or metadata changes

#### Scenario: Tampered source
- **WHEN** a catalogue source record has invalid authentication or is missing
- **THEN** source querying fails with an integrity error
