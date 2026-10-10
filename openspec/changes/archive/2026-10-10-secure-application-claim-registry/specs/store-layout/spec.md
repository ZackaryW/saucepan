# Spec Delta

## MODIFIED Requirements

### Requirement: Source metadata and output content have separate locations
Saucepan SHALL keep source definitions under `source/<source-uuid>.json`, optional provider working data under `source/<source-uuid>/`, live application and output metadata under `index/<uuid>/index.json`, application claim registries under `claims/<app-uuid>/index.json`, separate claim envelopes under `claims/<app-uuid>/<claim-uuid>.json`, and available acquired content under `store/<entry-uuid>/`. Claims and registered policies SHALL NOT remain embedded as a second mutable copy in the application index. Claim JSON files SHALL contain readable payloads with HMAC integrity tags as specified by claim-security. App and output records SHALL have distinguishable record types. Completed uninstall SHALL retire the live app record while retaining its operation receipt as specified by [application-lifecycle](../application-lifecycle/spec.md); output reclaim SHALL retain provenance as specified by [output-policies](../output-policies/spec.md). UUID and path validation SHALL prevent access outside the owning root.

#### Scenario: Application and output records coexist
- **WHEN** an application acquires content from a registered source
- **THEN** its app index identifies its claim registry, whose claim references an output index with an entry UUID resolving to the corresponding store directory
- **AND** the application does not require a store directory of its own

### Requirement: Legacy data is not implicitly accepted or migrated
Saucepan SHALL use a newly defined data contract without inheriting the old FORMAT_VERSION or compatibility decoders. Opening a legacy store or an earlier inline-claim layout SHALL fail explicitly with `unsupported_store` without changing its data. New stores SHALL have an explicit store UUID and contract discriminator. Missing or corrupt required claim metadata SHALL NOT be treated as an empty new store.

#### Scenario: An old encrypted central index is present
- **WHEN** initialization or opening detects a legacy Saucepan store
- **THEN** it reports that a separate new-format root is required and leaves the legacy store unchanged
