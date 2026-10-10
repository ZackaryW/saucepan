# source-query Specification

## Purpose

Find registered sources before acquiring content, with independently controlled public discovery and application permissions.

## Requirements

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

### Requirement: Source queries accept direct optional CLI filters
Using the default user-home root or an explicit override, `query-sources` SHALL accept no query arguments and behave as the empty source query. It SHALL accept optional `--uuid`, `--type`, `--repo`, `--origin` and `--url` filters combined with AND using the native source-query contract. No input file SHALL be required for direct filters. Type spellings SHALL be `local`, `url`, `git` and `github_release`. UUID parsing, literal case-sensitive matching, applicable provider fields and string bounds SHALL match saved JSON queries. No implicit scope or wildcard filter SHALL be introduced.

#### Scenario: Query without setup artifacts
- **WHEN** a caller runs `saucepan query-sources` on an initialized default store
- **THEN** the command returns the same result as an empty native query without reading a query file
- **AND** no app registration, scope or credential is required

#### Scenario: Filter a release repository directly
- **WHEN** a caller runs `saucepan query-sources --type github_release --repo owner/project`
- **THEN** the results match the equivalent JSON query with `type` and `repo`

#### Scenario: Validate literal flag values
- **WHEN** a filter has an invalid UUID or type, an empty or overlong string, or a repeated occurrence of the same flag
- **THEN** the command fails with `invalid_request` and no success stdout
- **AND** valid but contradictory filters yield an empty successful result

### Requirement: Saved source queries remain optional and unambiguous
`query-sources FILE` SHALL remain supported using the existing strict JSON query shape. A positional file SHALL be mutually exclusive with every direct query filter, even when their values agree. The command SHALL reject such combinations with `invalid_request` before trying to read the file or execute a query. Global options such as root, app identity and error format SHALL be allowed with either input form. Omitting the file and every filter SHALL never cause a stdin prompt or filesystem query-file lookup.

#### Scenario: Saved query parity
- **WHEN** a caller supplies a saved query file or equivalent direct flags with the same identity and unchanged catalogue
- **THEN** both commands return identical JSON results

#### Scenario: Reject implicit precedence
- **WHEN** a caller combines a missing or valid query file with `--repo owner/project`
- **THEN** argument validation fails rather than opening the file, merging filters or choosing a winner

### Requirement: Query identity is explicit and optional across input forms
Direct and file-based source queries without `--as-app` SHALL use anonymous source visibility without creating an application. Supplying `--as-app` SHALL use that application's existing credentials, discover action and source restrictions. Failed app authentication or authorization SHALL not fall back to anonymous discovery. The CLI SHALL NOT infer or synthesize an app identity. Existing explicit source-publication defaults and claim ownership requirements SHALL remain unchanged by the new input forms.

#### Scenario: Anonymous results stay public
- **WHEN** an unregistered caller queries a store containing both explicitly public and private sources using no input file
- **THEN** only explicitly public matching sources appear
- **AND** the query does not publish private sources or register an app

#### Scenario: App identity remains optional and meaningful
- **WHEN** an app supplies its UUID with direct filters
- **THEN** its permitted sources are searched under the same policy as file-based queries
- **AND** a missing required credential produces an authentication error without fallback

### Requirement: Source query help leads with the simplest discovery flow
Command help and source-query documentation SHALL show `saucepan query-sources` with no root override, app UUID or query file, followed by direct filters. Default-root initialization, explicit root overrides, app-bound queries, optional saved files and the separate claim-and-lease flow SHALL be explained. Documentation SHALL explain that an empty anonymous result can mean no sources have been published and SHALL NOT prescribe app registration as a prerequisite for anonymous discovery.

#### Scenario: First example works for an anonymous caller
- **WHEN** a user follows the first source-query example against an initialized store
- **THEN** no app UUID or JSON query file is required
- **AND** the documented result remains subject to explicit source visibility
