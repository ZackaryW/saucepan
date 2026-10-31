# Spec Delta

## MODIFIED Requirements

### Requirement: Legacy data is not implicitly accepted or migrated
Saucepan SHALL identify the current layout, flat policy resources with platform source records, through a distinct named store contract without inheriting the old FORMAT_VERSION or compatibility decoders. Opening a legacy store, earlier inline-claim layout, preceding nested-policy composition store or preceding flat-policy store whose source records may use the removed `git` type SHALL fail explicitly with unsupported_store before recovery, signing-key provisioning or data mutation. New stores SHALL have an explicit store UUID and contract discriminator. Missing or corrupt required claim or policy metadata SHALL NOT be treated as an empty new store. Known flat stores containing mixed inline and referenced policy authorities SHALL fail validation without fallback. No automatic conversion or in-place migration is provided.

#### Scenario: An old encrypted central index is present
- **WHEN** initialization or opening detects a legacy Saucepan store
- **THEN** it reports that a separate new-format root is required and leaves the legacy store unchanged

#### Scenario: Previous composition store is opened
- **WHEN** a root uses the preceding nested-policy contract
- **THEN** the binary reports unsupported_store with guidance to use a separate fresh root
- **AND** the source data, metadata, pending journal and credentials remain unchanged

#### Scenario: Previous flat-policy store is opened
- **WHEN** a root uses the preceding flat-policy contract
- **THEN** the binary reports unsupported_store with guidance to use a separate fresh root
- **AND** its source records, outputs and metadata remain unchanged
