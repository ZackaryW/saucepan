# encrypted-store Specification

## Purpose

Store sources, registered apps, their settings and initial filters, and scoped app records in authenticated encrypted indexes using an OS-keyring secret, with explicit isolated test construction.

## Requirements

### Requirement: Keyring custody and authenticated indexes
Production index encryption/authentication secrets SHALL be obtained from the OS keyring. Persisted full and source indexes SHALL be authenticated and encrypted. Ordinary opening SHALL NOT silently replace a missing key, recreate a missing established index, or fall back to plaintext. Unavailable keys and failed index authentication SHALL fail explicitly.

#### Scenario: Reopen established state
- **WHEN** the expected key is available and index authentication succeeds
- **THEN** Saucepan reads the previously recorded sources, registered apps, settings, initial filters, and scoped app records without consulting local TOML settings

#### Scenario: Unavailable or incorrect key
- **WHEN** the key is unavailable or cannot authenticate the existing index
- **THEN** opening fails without creating replacement authority or plaintext state

### Requirement: Explicit custom path and key for tests
Explicit test construction SHALL accept a custom store path and supplied key instead of OS-keyring custody. It SHALL retain the same index, origin, snapshot, and sub-index verification behavior. Test configuration SHALL NOT be an automatic production fallback or persist the supplied secret in the index tree.

#### Scenario: Independent test store
- **WHEN** a test opens its custom store with the correct supplied key
- **THEN** it accesses only that store and does not require a production keyring enrollment

#### Scenario: Wrong test key
- **WHEN** the supplied key does not match the custom store
- **THEN** authentication fails without trying the production keyring or initializing replacement state

### Requirement: Trusted-user security boundary
Saucepan SHALL use the OS keyring and standard cryptographic library primitives without implementing custom OS ACLs, accounts, process authentication, elevation, or a permission-administration service. Encrypted indexes and HMAC proofs SHALL NOT be described as publisher signatures or hostile same-user process isolation.

#### Scenario: Normal user operation
- **WHEN** Saucepan acquires content or verifies an app sub-index
- **THEN** it uses normal user-level filesystem/keyring access without adding custom OS security management

### Requirement: Native secret providers preserve the same store behavior
Saucepan SHALL support production secret access through Windows Credential Store on Windows, Keychain on macOS, and Secret Service on Linux while preserving the same encrypted-index and HMAC behavior. Unavailable or locked providers SHALL produce an explicit error. A missing Linux secret service SHALL NOT activate plaintext storage, an automatically created file key, or the explicit custom-key test mode. Custom test stores SHALL exercise the same core behavior independently of native provider availability.

#### Scenario: Native provider on a supported OS
- **WHEN** a supported platform's native secret provider is available and supplies the correct key
- **THEN** the store opens with the same registered app, source, policy, and verification behavior as on the other supported platforms

#### Scenario: Headless Linux without Secret Service
- **WHEN** production opening cannot access Secret Service
- **THEN** it fails explicitly without substituting another secret-storage mechanism or weakening authentication
