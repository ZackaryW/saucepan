## MODIFIED Requirements

### Requirement: Source metadata and output content have separate locations
Saucepan SHALL keep source definitions under source/<source-uuid>.json, optional provider working data under source/<source-uuid>/, live application and output metadata under index/<uuid>/index.json, application claim registries under claims/<app-uuid>/index.json, separate claim envelopes under claims/<app-uuid>/<claim-uuid>.json, standalone policy definitions under policies/<policy-uuid>.json, and available acquired content under store/<entry-uuid>/. Persistent retrieval and acquisition-check records SHALL retain their distinct index record types. Applications, sources and claims SHALL retain policy references rather than embedded authoritative policy definitions or defaults/override bundles. Policy records SHALL participate in verified catalog membership and recoverable publication. Metadata SHALL remain readable and verified using the store basic checksums or keyring-backed HMAC tags. App and output records SHALL have distinguishable types. Completed uninstall SHALL retire the live app record while retaining its operation receipt; output reclaim SHALL retain provenance. Policy definitions referenced by historical records SHALL not be deleted by uninstall or reclaim. UUID and path validation SHALL prevent access outside the owning root.

#### Scenario: Application and output records coexist
- **WHEN** an application acquires content from a registered source
- **THEN** its app index identifies its claim registry, whose claim references an output index with an entry UUID resolving to the corresponding store directory
- **AND** the application does not require a store directory of its own

### Requirement: Legacy data is not implicitly accepted or migrated
Saucepan SHALL identify the flat-policy layout through a distinct named store contract without inheriting the old FORMAT_VERSION or compatibility decoders. Opening a legacy store, earlier inline-claim layout or preceding nested-policy composition store SHALL fail explicitly with unsupported_store before recovery, signing-key provisioning or data mutation. New flat stores SHALL have an explicit store UUID and contract discriminator. Missing or corrupt required claim or policy metadata SHALL NOT be treated as an empty new store. Known flat stores containing mixed inline and referenced policy authorities SHALL fail validation without fallback. No automatic conversion or in-place migration is provided.

#### Scenario: An old encrypted central index is present
- **WHEN** initialization or opening detects a legacy Saucepan store
- **THEN** it reports that a separate new-format root is required and leaves the legacy store unchanged

#### Scenario: Previous composition store is opened
- **WHEN** a root uses the preceding nested-policy contract
- **THEN** the binary reports unsupported_store with guidance to use a separate fresh root
- **AND** the source data, metadata, pending journal and credentials remain unchanged
