# Spec Delta

## ADDED Requirements

### Requirement: Recovery artifacts preserve record integrity and secret custody
Journals, prepared receipts, retired records and persisted temporary metadata SHALL carry verifiable HMAC integrity protection binding their identities and generation. Claim metadata SHALL remain readable; bearer tokens and signing keys SHALL never be written into these artifacts. Store opening SHALL verify committed intent before replay, fail closed on corruption, and replay valid intent idempotently without client credentials, network access or new preparation. Preparation retries and operation inspection SHALL enforce applicable app credentials and action/scope restrictions. Missing or corrupt claim state SHALL NOT be interpreted as zero retention demand. Store catalogs, record writes and retirements SHALL participate in one coordinated recoverable transition.

#### Scenario: Inspect interrupted files
- **WHEN** an operation stops at a publication boundary
- **THEN** recovery metadata remains readable and integrity-verifiable but contains no bearer tokens or signing keys

#### Scenario: Tampered intent is replayed
- **WHEN** store opening encounters a changed committed transaction with an invalid HMAC
- **THEN** it reports `integrity_error` without accepting the proposed claim or binding changes

#### Scenario: Valid intent survives a client exit
- **WHEN** an app commits intent and its client exits with its credential
- **THEN** recovery can complete that verified intent without reacquiring content or obtaining the client secret
