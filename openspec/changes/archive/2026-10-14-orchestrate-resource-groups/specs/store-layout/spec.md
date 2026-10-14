# Spec Delta

## ADDED Requirements

### Requirement: Registered groups participate in verified recoverable metadata
Group definitions and selected generations SHALL be stored under groups/<group-uuid>/index.json as typed catalog members, with validated UUID/owner identity and the store's existing basic or keyring integrity mode. Missing, corrupt or mismatched expected group members SHALL fail integrity validation rather than becoming empty registrations. Group generations and participating claim/check records SHALL use the existing recoverable publication authority. No second independent group index SHALL become an authority. Existing source working data, output directories and claim registries SHALL retain their locations. Stores without groups SHALL not require a rewrite; legacy contracts SHALL remain rejected without conversion or fallback.

#### Scenario: Group metadata is modified externally
- **WHEN** a group envelope no longer matches verified catalog membership
- **THEN** lookup or preparation fails integrity validation without blessing the edited record

#### Scenario: Basic-mode group lifecycle
- **WHEN** a local caller registers and prepares a group in a basic store
- **THEN** group and selection records are checksum-verified without requiring native key custody

### Requirement: Durable pins are verified independent metadata members
Pin records SHALL live under pins/<pin-uuid>.json with validated identity/owner, active or released state and verified catalog membership under the store's basic/keyring mode. Pin creation and release SHALL use the recoverable metadata transaction authority. Missing, edited or corrupt expected records SHALL fail integrity validation before reclaim can discard their protection. Released records SHALL remain receipts preventing accidental key reuse. App retirement and group deletion SHALL preserve pin records and immutable referenced policy evidence. Stores without pins SHALL require no migration or rewrite; no legacy decoder or fallback SHALL be introduced.

#### Scenario: A pin file is deleted outside Saucepan
- **WHEN** the verified catalog still expects a pin whose file is missing
- **THEN** reclaim fails integrity validation instead of treating the pinned output as unprotected

#### Scenario: Basic-mode retention survives reopening
- **WHEN** a basic store with an active pin is reopened
- **THEN** its pin is verified and applied without native key custody
