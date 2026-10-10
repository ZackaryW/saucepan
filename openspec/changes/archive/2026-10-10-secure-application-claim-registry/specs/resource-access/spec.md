# Spec Delta

## MODIFIED Requirements

### Requirement: Claim-based access selects and protects one consistent output
A caller SHALL be able to acquire a resource lease using an app UUID and claim UUID in a single coordinated operation, after enforcing the application's configured credential requirement and source/claim action policy. App-scoped exact-output access SHALL enforce the same policy and SHALL NOT fall back to unrestricted store administration. An optional relative view SHALL be validated against the selected output manifest and containment boundary before exposing its directory. A claim lease SHALL protect the whole underlying output and SHALL NOT create another claim or automatically recover unavailable content. Concurrent claim updates SHALL produce a lease on either the complete old selection or the complete new selection. The returned lease SHALL identify which output it protects and SHALL remain attached to that output after the claim changes or is released. Listing filters SHALL NOT implicitly create or grant a claim.

#### Scenario: A claim updates during access
- **WHEN** an application updates a claim while another consumer obtains a lease for that claim
- **THEN** the consumer receives one valid protected output with its matching directory
- **AND** any subsequent update leaves that lease's output identity unchanged

#### Scenario: Native consumer retains a covered view during reclaim
- **WHEN** a Rust caller holds a lease for a contained claim view while another process releases the claim and attempts reclaim
- **THEN** the entire selected output remains protected until that guard and all other applicable protections end

#### Scenario: Native view guard is dropped
- **WHEN** the Rust caller drops the last guard protecting an otherwise reclaimable output
- **THEN** a later reclaim can delete its content without changing retained claim provenance
