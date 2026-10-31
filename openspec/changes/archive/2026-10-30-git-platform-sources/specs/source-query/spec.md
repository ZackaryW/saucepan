# Spec Delta

## MODIFIED Requirements

### Requirement: Source queries are independent of claims and scopes
Queries SHALL accept optional exact `uuid`, `type`, `repo`, `origin` and `url` fields combined with AND. Supported stored types SHALL be `local`, `url`, `git_repo`, `github_repo`, `github_release`, `github_raw`, `git_raw` and `github_source`. `repo` SHALL match the repository strings of `github_repo`, `github_release`, `github_raw`, `git_raw` and `github_source` sources, `origin` `git_repo` origins and `url` URL sources, case-sensitively against stored values without fetching or canonicalizing inputs. Missing applicable fields SHALL not match. Empty objects SHALL match all permitted sources including unclaimed sources. Unknown fields, invalid UUIDs or types, and empty or over-4096-byte string filters SHALL fail validation. Claim scope SHALL not be a source query field.

#### Scenario: Discover before acquiring
- **WHEN** an application searches a registered release repository without any claims or downloaded outputs
- **THEN** the matching source is returned with its UUID and source definition

#### Scenario: Strict filters
- **WHEN** a query supplies an unknown field, a scope, an invalid type, or an invalid filter string
- **THEN** it fails even when the catalogue is empty

#### Scenario: Removed git type
- **WHEN** a query supplies type `git`
- **THEN** it fails validation as an invalid type

#### Scenario: Repository filter spans GitHub types
- **WHEN** `github_repo` and `github_source` sources for `owner/project` are registered and a query supplies `repo` `owner/project` without `type`
- **THEN** both sources are returned

### Requirement: Source queries accept direct optional CLI filters
Using the default user-home root or an explicit override, `query-sources` SHALL accept no query arguments and behave as the empty source query. It SHALL accept optional `--uuid`, `--type`, `--repo`, `--origin` and `--url` filters combined with AND using the native source-query contract. No input file SHALL be required for direct filters. Type spellings SHALL be `local`, `url`, `git_repo`, `github_repo`, `github_release`, `github_raw`, `git_raw` and `github_source`. UUID parsing, literal case-sensitive matching, applicable provider fields and string bounds SHALL match saved JSON queries. No implicit scope or wildcard filter SHALL be introduced.

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
