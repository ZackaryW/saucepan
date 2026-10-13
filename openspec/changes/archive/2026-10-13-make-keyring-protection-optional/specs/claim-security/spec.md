## RENAMED Requirements

- FROM: `### Requirement: Readable records have keyed integrity protection`
- TO: `### Requirement: Readable records follow the store integrity mode`

## MODIFIED Requirements

### Requirement: Readable records follow the store integrity mode
Saucepan SHALL store claim and policy metadata as readable JSON without encryption, verified according to the store's selected integrity mode. `basic` SHALL use SHA-256 checksums without signing keys; `keyring` SHALL use HMAC-SHA256 tags with native key custody. Verification SHALL bind record kind, store/application/member identity as applicable, generation and the exact persisted payload bytes. Basic records SHALL additionally bind their explicit contract and mode. Parsing SHALL be bounded and reject duplicate or unknown envelope fields. Verification SHALL occur before payloads are used. A modified or reformatted payload with a mismatching integrity value SHALL fail; Saucepan SHALL NOT silently regenerate that value. Verified catalogs and registry membership SHALL detect missing or substituted live records relative to the verified catalog. Source definitions, output provenance, bindings and policies SHALL use the same store-wide mode. Protected metadata SHALL NOT be accepted using basic verification after an HMAC or custody failure. Basic verification SHALL NOT be represented as resistance to deliberate rewriting of records and checksums together.

#### Scenario: A user reads an ordinary claim file
- **WHEN** a claim record is opened as JSON in either mode
- **THEN** its metadata is readable and no signing key is present

#### Scenario: An editor changes a policy and its checksum
- **WHEN** a caller edits a keyring-protected record and recomputes its unkeyed checksum without the signing key
- **THEN** the HMAC fails and Saucepan rejects the modified state with `integrity_error`

#### Scenario: A claim or source record is substituted
- **WHEN** a record is transplanted across identities, deleted from expected membership, or its source location is edited without updating the required integrity values
- **THEN** metadata access fails integrity validation rather than using altered policy inputs

#### Scenario: Basic metadata is corrupted
- **WHEN** a basic record's payload or checksum changes without a matching verified catalog and record checksum
- **THEN** access reports `integrity_error` without repairing or accepting the record

#### Scenario: A basic envelope is inserted into a protected store
- **WHEN** a protected catalog, member or transaction is replaced with a basic envelope
- **THEN** access fails without retrying it under basic verification

### Requirement: Application credentials and access restrictions are independent
Credential enforcement SHALL be opt-in per registered application and disabled by default in both integrity modes. Enabled applications SHALL require valid app-bound proof in addition to policy permission for app operations. New credential verifiers SHALL be bound to store identity, application identity and credential generation without depending on a store signing key or native keyring. Existing supported protected credential verifiers SHALL retain their meaning until explicitly changed. Disabling credentials SHALL remove the proof requirement only; source, action, claim and scope restrictions SHALL still apply. Selecting basic integrity SHALL NOT disable credentials, publish sources/scopes, or bypass restrictions. An explicit app-scoped request with proof disabled SHALL be evaluated against that app's registered policies. An app UUID or context query match SHALL NOT grant additional rights. Credential changes SHALL be explicit revision-checked registration operations; changing an enabled mode SHALL require valid current proof. Rotation SHALL invalidate old tokens without revoking existing native leases.

#### Scenario: Credential-free app requests a forbidden source
- **WHEN** an app with credentials disabled requests a source outside its registered allowance in either integrity mode
- **THEN** acquisition fails with `access_denied` before source or claim effects

#### Scenario: Credential-free app requests permitted content
- **WHEN** an explicit app-scoped request satisfies all its registered restrictions
- **THEN** the operation requires no app token

#### Scenario: Proof does not override restrictions
- **WHEN** an authenticated app requests an action its policy disallows
- **THEN** the operation fails with `access_denied`

#### Scenario: Missing proof for a protected app
- **WHEN** an app with credentials enabled issues a query or mutation without valid proof in either integrity mode
- **THEN** it fails with `authentication_required` or `authentication_failed` without restricted data or effects

#### Scenario: Independent credentials work without custody
- **WHEN** an app enables, uses and rotates credentials in a basic store on a host without keyring access
- **THEN** valid proof succeeds, wrong or old proof fails, and no operation contacts a keyring
- **AND** the same token presented for another store or app does not satisfy that registration's verifier

### Requirement: Signing keys and optional credentials have separate custody
Only keyring-protected stores SHALL provision an HMAC master key in OS-backed keyring under a store-specific identity, separately from JSON files. Missing or unavailable protected keys SHALL produce `key_unavailable` without generating replacement keys for existing records or falling back to basic acceptance. Basic stores SHALL NOT provision, load, retire, or require a signing key. Test key providers SHALL require explicit injection for protected fixtures. Optional bearer tokens SHALL be random, distinct from signing keys, verified using constant-time library primitives, and delivered only through a chosen secret destination. Persisted credential verifiers SHALL explicitly distinguish independent bearer proof from existing signing-key-dependent proof; verification SHALL NOT try alternative algorithms after a failed proof. Normal results, logs, records, journals and sidecars SHALL contain neither signing keys nor bearer tokens. Registration retries SHALL NOT redisclose tokens. CLI proof SHALL use a credential file rather than a literal command-line token.

#### Scenario: Store key is unavailable
- **WHEN** an existing protected store cannot obtain its HMAC key
- **THEN** access fails with `key_unavailable` and signed state is unchanged

#### Scenario: Credential-free records are verified
- **WHEN** a caller accesses an app whose credential enforcement is disabled
- **THEN** Saucepan still verifies metadata according to its store mode without asking for an app token

#### Scenario: Current protected credentials remain valid
- **WHEN** the new binary opens a current-format protected store with existing signing-key-dependent credential verifiers
- **THEN** existing tokens retain their validity without reissue or metadata conversion
- **AND** a later explicit rotation creates an independent verifier and rejects the old token

### Requirement: Integrity and policy guarantees have explicit local boundaries
Saucepan SHALL document that basic checksums provide consistency and corruption detection, not authentication against a writer who can update records and checksums together. Basic app credentials and policies SHALL govern Saucepan API operations without promising protection against filesystem writers. HMAC SHALL be documented as modification detection without the signing key, not encryption, prevention of filesystem writes, protection against a signing-key holder, or detection of a complete earlier valid store snapshot without an external anchor. OS permissions SHALL remain responsible for filesystem protection. Store mode detection SHALL NOT be presented as an external trust anchor against wholesale store replacement. Trusted local maintenance SHALL verify committed intent and retention metadata according to the selected mode without requiring app tokens; it SHALL NOT expose restricted metadata through ordinary app APIs. Shims SHALL launch only the verified committed binding under a narrow execution authority, without embedding general app credentials. Explicit trusted store administration SHALL be distinct from app-scoped direct-output access.

#### Scenario: Valid committed intent survives client exit
- **WHEN** a client exits after committing a valid transition in either integrity mode
- **THEN** maintenance can verify and finish it without the client token and without preparing new acquisition

#### Scenario: An altered binding is launched
- **WHEN** a shim encounters a binding that fails its store's required metadata verification
- **THEN** it rejects the binding before execution

## ADDED Requirements

### Requirement: Native custody verification is independently selectable
Ordinary core and integration acceptance SHALL execute without OS keyring access, including real manager and shim processes. Deterministic protected-integrity tests SHALL remain available without a native service through explicitly injected test custody. Actual OS custody verification SHALL be a separately selected suite, covering round-trip, cleanup, concurrent stores and cross-executable manager/shim access. When explicitly selected, unavailable custody or failed protected behavior SHALL fail with bounded subprocess waits and secret-free diagnostics, not silently pass or skip. Reports SHALL distinguish unrequested native checks from successfully verified native behavior. Hosted checks SHALL retain manual/release-only triggers and platform selection; ordinary releases SHALL NOT require a native credential service. Explicitly selected native checks SHALL block release on failure.

#### Scenario: Ordinary suite runs on a host without a keyring service
- **WHEN** the normal integration and real-build acceptance suite executes without native custody selected
- **THEN** core functionality is exercised in basic stores without keyring setup
- **AND** protected integrity remains tested with explicit deterministic custody while native verification is reported as not run

#### Scenario: Native checks were explicitly requested
- **WHEN** native custody checks are selected and a process cannot obtain its store key
- **THEN** the suite fails within its configured deadline with a diagnostic and does not report protected mode as verified

#### Scenario: A protected manager and shim share one root
- **WHEN** the selected native suite initializes a protected root and accesses it from the test executable, manager and actual shim
- **THEN** each verifies the same committed state using native custody and owned fixture keys are retired after the test
