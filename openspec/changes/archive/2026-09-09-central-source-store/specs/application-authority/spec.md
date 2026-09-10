## Purpose

Keep each app's index view inside the same Saucepan: record the entries its operations touch, apply its initial filters, and authenticate the scoped view with the recorded HMAC when authoritative.

## ADDED Requirements

### Requirement: Register and configure apps in the encrypted index
Saucepan SHALL register apps and persist their identity, settings, and initial filters in the central encrypted index. Configuration operations SHALL update the corresponding app record. App operations SHALL resolve the registered app and read its settings from that authenticated record, in both ordinary and authoritative modes. The new implementation SHALL NOT load or require `saucepan.toml`, use it as a settings override/fallback, or persist a second plaintext app-settings store. App registration SHALL NOT introduce owner credentials, OS/path identity enrollment, or role/action/destination grants.

#### Scenario: Registered app without a TOML file
- **WHEN** an app is registered and configured in the central encrypted index and has no local TOML settings file
- **THEN** Saucepan uses its stored settings and initial filters for its operations

#### Scenario: Change one app's settings
- **WHEN** app A's configuration is updated in the encrypted index
- **THEN** subsequent A operations read the updated settings while app B's configuration remains unchanged

#### Scenario: Legacy settings file is present
- **WHEN** a local saucepan.toml conflicts with the registered app's settings
- **THEN** the new implementation uses the encrypted app record and does not read the TOML file as an override or fallback

### Requirement: The encrypted index defines app sub-index authority
Saucepan SHALL record app operations and the entries they touch in its central index under the calling app context. The app's sub-index SHALL present only its touched entries matching the app's initial filtering setup. It SHALL remain a scoped view of the same Saucepan index, not a separately issued or synchronized index. In authoritative mode the encrypted index SHALL record the corresponding scopes and HMAC. Application identity, scope information, and selected data SHALL be authenticated together. Editing presented data SHALL NOT change that authenticated record. Scopes SHALL NOT define resource grants, a source/recipe/artifact permission hierarchy, or action/destination permissions.

#### Scenario: Two application views
- **WHEN** applications A and B have touched different entries through Saucepan operations
- **THEN** each app sees its own touched entries matching its initial filters, with untouched or filtered-out entries omitted

#### Scenario: Touch through shared cache reuse
- **WHEN** app A acquires content already cached through app B
- **THEN** Saucepan records A's operation and association with that entry without duplicating the source or making B's other entries visible to A

#### Scenario: Initial filters still apply
- **WHEN** an app has touched an entry that does not match its configured initial filters
- **THEN** its presented sub-index omits that entry

#### Scenario: Listing does not claim the entire store
- **WHEN** an app lists its index
- **THEN** Saucepan presents its existing scoped view without recording every global index entry as touched by that app

#### Scenario: Overlapping selections
- **WHEN** applications A and B touch some of the same entries and their filters include those entries
- **THEN** that data may appear in both sub-indexes without creating source ownership or resource grants

#### Scenario: Changed scope declaration
- **WHEN** an application edits its copy of the scope or adds entries outside it
- **THEN** the changed sub-index does not verify against the authenticated full-index record

### Requirement: Applications establish authority through verification
Saucepan SHALL expose a method that verifies an app's scoped index data against its authenticated payload and corresponding encrypted record. The app SHALL obtain its verification result and scoped data without receiving the full index or master secret. Verification SHALL use the secret-backed HMAC; merely comparing public hash strings SHALL NOT establish authority. Normal index operations SHALL maintain consistent app associations, scopes, and HMAC data. Implementations SHALL NOT add a separate issuance, refresh, synchronization, or permission-revocation service for sub-index copies.

#### Scenario: Normal operation updates the app view
- **WHEN** an app successfully acquires another entry matching its initial filters
- **THEN** Saucepan records the operation and exposes the entry in that app's same-index view with consistent authentication data, without requiring a separate sub-index refresh command

#### Scenario: Valid app sub-index
- **WHEN** the app supplies its valid caller token and the current scoped data matches the recorded HMAC
- **THEN** verification succeeds and identifies the scoped data as authenticated

#### Scenario: Substituted app proof
- **WHEN** app A's sub-index is paired with app B's proof or claimed identity
- **THEN** verification fails

#### Scenario: Altered selected data
- **WHEN** the app presents modified selected data with the original proof
- **THEN** HMAC verification fails rather than authenticating the modified view

### Requirement: Caller tokens remain stable while data authentication changes
Saucepan SHALL establish a caller token for the registered app and verify its binding to that app and store separately from the HMAC authenticating current scoped data. Normal index operations, app settings updates, and touched-entry changes SHALL NOT replace the caller token. Saucepan SHALL update the scoped-data HMAC consistently with changed authenticated data. Possessing a valid caller token SHALL NOT make altered or stale data pass current-data verification. Token stability SHALL NOT require a refresh command or per-operation marker replacement.

#### Scenario: Token reused after acquisition
- **WHEN** app A uses its token to acquire another entry and then calls Saucepan again with the same token
- **THEN** the token remains valid for A and Saucepan verifies A's updated scoped data using the maintained current HMAC

#### Scenario: Token reused after configuration changes
- **WHEN** A's settings or filters are changed through Saucepan
- **THEN** subsequent calls use the new recorded configuration with the same caller token and consistent scoped-data authentication

#### Scenario: Stable token with altered data
- **WHEN** a valid caller token accompanies scoped data that does not match the current authenticated record
- **THEN** data verification fails despite the token being valid

### Requirement: The app marker carries the stable caller token or reference
When `.saucepanhash` is used, it SHALL carry the stable caller token/reference for the app, not a changing data HMAC requiring replacement after index operations. It SHALL NOT contain the master secret or act as an RSA key, owner credential, OS-account credential, or application-root enrollment certificate. A claimed app identifier alone SHALL NOT count as verification.

#### Scenario: Marker verification
- **WHEN** an app supplies its `.saucepanhash` proof/reference
- **THEN** the verification method checks the corresponding sub-index and encrypted record instead of granting authority from the marker's presence alone

### Requirement: App index consumers use the verified selection
An app's authoritative lookup or setup SHALL verify its sub-index before consuming its index data. App-facing index results SHALL contain only data selected by that sub-index. Failed verification SHALL NOT fall back to the full index. Scope selection SHALL NOT introduce permission checks on acquisition, dependency resolution, caching, or mirrors. Ordinary app acquisition SHALL use the registered app's encrypted configuration without requiring resource/action/destination grants.

#### Scenario: Lookup outside the selected data
- **WHEN** an app looks up an index entry absent from its verified sub-index
- **THEN** the lookup reports no selected entry without exposing a full-index fallback result

#### Scenario: Acquisition dependencies remain acquisition inputs
- **WHEN** a recipe requires submodule or LFS content to produce a complete artifact
- **THEN** acquisition follows the complete-content contract without requiring a scope grant for each dependency or adding unselected index records to the app's view

#### Scenario: Missing or invalid proof
- **WHEN** an authoritative request lacks a valid sub-index proof
- **THEN** it fails without returning full-index data or switching to ordinary acquisition

#### Scenario: Ordinary acquisition
- **WHEN** a registered app makes an ordinary acquisition
- **THEN** Saucepan reads its centrally stored settings and performs index, origin, policy, and content handling without per-artifact approvals or permission administration

### Requirement: Authority formats are versioned and store-bound
Authoritative app calls SHALL supply the app identity, its proof/reference, and the requested operation through the same logical interface on Windows, macOS, and Linux. The request/proof format SHALL carry an explicit version. Saucepan SHALL reject unsupported versions and unrecognized authority restrictions instead of treating them as unrestricted requests. Proofs SHALL be verified against the corresponding store record and secret; running on a supported OS SHALL NOT make another store's proof valid. Read-only authority and delegated issuance are not implemented by this requirement.

#### Scenario: Unknown authority restriction
- **WHEN** a request carries an unsupported format version or authority restriction
- **THEN** Saucepan reports incompatibility without executing it under unrestricted authority

#### Scenario: Proof from another store
- **WHEN** a proof is presented to a store whose authenticated app record or secret does not match
- **THEN** verification fails even though both stores use supported operating systems
