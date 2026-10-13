## MODIFIED Requirements

### Requirement: Access-time verification is explicit
Resource access SHALL support `metadata` and `full` verification modes. Ordinary path resolution and routine managed launch SHALL default to metadata mode. Metadata mode SHALL validate record identity, output kind, ready state, root existence/type, and filesystem containment without traversing or reading output payload files. A selected managed executable SHALL additionally be declared in the output and present as a regular contained file. Unknown modes SHALL fail before launching a consumer.

Both access-time modes SHALL verify the store-selected metadata integrity contract in [claim-security](../claim-security/spec.md): checksums in basic stores and HMAC in keyring-protected stores. Access-time mode SHALL NOT select or weaken the store's integrity mode. Metadata mode SHALL NOT skip required record verification; its reduced work concerns output payload scanning, not metadata integrity or registered launcher-file verification.

#### Scenario: Looking up a large output
- **WHEN** a caller resolves a ready output using the default mode with no pending recovery
- **THEN** Saucepan returns its exact directory without scanning the output tree or hashing payload bytes
- **AND** the result identifies verification as `metadata`, not full content verification

#### Scenario: Unsafe or unavailable target
- **WHEN** an entry is pending reclaim, reclaimed, missing its root, or resolves through a forbidden filesystem link
- **THEN** metadata access fails without refreshing or repairing the source
