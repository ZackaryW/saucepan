# Spec Delta

## Purpose

Verify readable claim metadata for unauthorized modification and apply configurable source, claim and scope policies consistently across Saucepan operations.

## ADDED Requirements

### Requirement: Readable records have keyed integrity protection
Saucepan SHALL store claim and policy metadata as readable JSON with HMAC-SHA256 integrity tags, without encryption. Verification SHALL bind record kind, store/application/member identity as applicable, generation and the exact persisted payload bytes. Record parsing SHALL be bounded and reject duplicate or unknown envelope fields. Tags SHALL be verified before payloads are trusted. A modified or reformatted signed payload SHALL fail verification; Saucepan SHALL NOT silently re-sign mismatched records. Authenticated catalogs and registry membership SHALL detect missing or substituted live records. Source definitions, output provenance, bindings, and policies used to make access decisions SHALL also be integrity-protected.

#### Scenario: A user reads an ordinary claim file
- **WHEN** a claim record is opened as JSON
- **THEN** its metadata is readable and the HMAC key is absent

#### Scenario: An editor changes a policy and its checksum
- **WHEN** a caller edits a record and recomputes its unkeyed checksum without the signing key
- **THEN** the HMAC fails and Saucepan rejects the modified state with `integrity_error`

#### Scenario: A claim or source record is substituted
- **WHEN** a record is transplanted across identities, deleted from expected membership, or its source location is edited
- **THEN** metadata access fails integrity validation rather than trusting altered policy inputs

### Requirement: Application credentials and access restrictions are independent
Credential enforcement SHALL be opt-in per registered application and disabled by default. Enabled applications SHALL require valid app-bound proof in addition to policy permission for app operations. Disabling credentials SHALL remove the proof requirement only; source, action, claim and scope restrictions SHALL still apply. An explicit app-scoped request with proof disabled SHALL be evaluated against that app's registered policies. An app UUID or context query match SHALL NOT grant additional rights. Credential changes SHALL be explicit revision-checked registration operations; changing an enabled mode SHALL require valid current proof. Rotation SHALL invalidate old tokens without revoking existing native leases.

#### Scenario: Credential-free app requests a forbidden source
- **WHEN** an app with credentials disabled requests a source outside its registered allowance
- **THEN** acquisition fails with `access_denied` before source or claim effects

#### Scenario: Credential-free app requests permitted content
- **WHEN** an explicit app-scoped request satisfies all its registered restrictions
- **THEN** the operation requires no app token

#### Scenario: Proof does not override restrictions
- **WHEN** an authenticated app requests an action its policy disallows
- **THEN** the operation fails with `access_denied`

#### Scenario: Missing proof for a protected app
- **WHEN** a protected app's query or mutation lacks valid proof
- **THEN** it fails with `authentication_required` or `authentication_failed` without restricted data or effects

### Requirement: One operation policy governs application access
App-facing lookup, acquisition, claim ensure/reuse, leasing, recovery, update, release, activation and removal SHALL evaluate caller, action and resource against registered source and claim/scope permissions. Omitted restrictions SHALL permit supported actions within the app's ownership boundary; an explicitly empty allowance SHALL permit none. Configured dimensions SHALL intersect, and a permitted source SHALL NOT override a denied action or scope. Cache hits, covered views and direct UUID lookup SHALL NOT bypass policy. Presentation filters SHALL remain distinct from authorization. Restricted app calls SHALL NOT edit their own registration to broaden authority or select trusted administrative mode. Policy changes SHALL use the explicit local store-administration boundary with revision checking and applicable proof.

#### Scenario: A cached output would bypass acquisition restrictions
- **WHEN** an app requests an already-cached output belonging to a forbidden source
- **THEN** app-scoped access is denied even if no download is required

#### Scenario: An empty scope allowance is configured
- **WHEN** an app has an explicitly empty allowed-scope set
- **THEN** it cannot create or access claims in arbitrary scopes

#### Scenario: An app attempts to broaden its own policy
- **WHEN** a restricted app operation attempts to modify registered source allowances
- **THEN** it is denied without altering the registration

### Requirement: Signing keys and optional credentials have separate custody
The HMAC master key SHALL reside in OS-backed keyring under a store-specific identity, separately from JSON files. Missing or unavailable keys SHALL produce `key_unavailable` without generating replacement keys for existing records or falling back to unsigned acceptance. Test key providers SHALL require explicit injection. Optional bearer tokens SHALL be random, distinct from signing keys, verified using constant-time library primitives, and delivered only through a chosen secret destination. Normal results, logs, records, journals and sidecars SHALL contain neither signing keys nor bearer tokens. Registration retries SHALL NOT redisclose tokens. CLI proof SHALL use a credential file rather than a literal command-line token.

#### Scenario: Store key is unavailable
- **WHEN** an existing store cannot obtain its HMAC key
- **THEN** access fails with `key_unavailable` and signed state is unchanged

#### Scenario: Credential-free records are verified
- **WHEN** a caller accesses an app whose credential enforcement is disabled
- **THEN** Saucepan still verifies metadata using the store key without asking the caller for an app token

### Requirement: Integrity and policy guarantees have explicit local boundaries
Saucepan SHALL document that HMAC provides modification detection, not encryption, prevention of filesystem writes, protection against a signing-key holder, or detection of a complete earlier valid store snapshot without an external anchor. OS permissions SHALL remain responsible for filesystem protection. Trusted local maintenance SHALL verify committed intent and retention metadata without requiring app tokens; it SHALL NOT expose restricted metadata through ordinary app APIs. Shims SHALL launch only the verified committed binding under a narrow execution authority, without embedding general app credentials. Explicit trusted store administration SHALL be distinct from app-scoped direct-output access.

#### Scenario: Valid committed intent survives client exit
- **WHEN** a client exits after committing a valid HMAC-protected transition
- **THEN** maintenance can finish it without the client token and without preparing new acquisition

#### Scenario: An altered binding is launched
- **WHEN** a shim encounters an externally modified binding
- **THEN** it rejects the binding before execution
