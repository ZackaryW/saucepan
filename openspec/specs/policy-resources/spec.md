# policy-resources Specification

## Purpose

Define independently reusable policy resources and explicit associations so callers can compose Saucepan behavior without duplicating authority or requiring persistent setup for fresh operations.

## Requirements

### Requirement: Policy definitions have immutable typed identities
Each policy definition SHALL have a UUID, a supported type and strictly validated type-specific fields. Definitions SHALL NOT contain other policy references or nested policy catalogs. Registering an existing UUID with equivalent typed content SHALL be idempotent; registering different content under that UUID SHALL fail with `policy_conflict`. A changed policy SHALL require a new UUID and explicit adoption. There SHALL be no policy-definition mutation or deletion operation in this contract. Registration alone SHALL NOT attach a policy or change any consumer.

#### Scenario: Change a shared revision policy
- **WHEN** an administrator registers a latest-five policy under a new UUID while two claims reference a latest-three policy
- **THEN** both claims retain latest-three until each explicitly adopts the new UUID
- **AND** attempting to overwrite the old UUID fails without changing either claim

### Requirement: Policy definitions can be looked up directly by UUID
Native and CLI interfaces SHALL support exact policy lookup by UUID. The CLI SHALL provide `saucepan policy POLICY_UUID` using the default store or an explicit root, without requiring a query file. Successful lookup SHALL return the full verified typed definition, including UUID, type and type-specific settings, rather than an effective-policy projection or a metadata file path. Trusted local store inspection SHALL not require app registration. An explicitly app-bound lookup SHALL enforce applicable credentials and require an authorized readable resource association to that definition; knowing a UUID SHALL NOT bypass that boundary or trigger administrative fallback. Malformed or unknown caller UUIDs SHALL produce `invalid_request`; a policy expected by the verified catalog but missing or corrupt SHALL produce `integrity_error`. Errors SHALL emit no success result. Lookup SHALL NOT fetch provider content, create claims, adopt policies or alter definitions; ordinary store-open recovery retains its existing contract. Native caller-owned catalogs SHALL also permit exact UUID lookup without opening a store.

#### Scenario: Inspect a referenced revision policy
- **WHEN** a local caller runs `saucepan policy POLICY_UUID` for a registered latest-three definition
- **THEN** the result contains that UUID, type revision_policy, history latest and keep 3
- **AND** no query file, app registration, provider request or policy adoption is needed

#### Scenario: Unknown policy differs from missing catalog data
- **WHEN** an otherwise authorized caller looks up a UUID absent from the catalog
- **THEN** lookup reports invalid_request without a success payload
- **AND** a missing file for an expected catalog member instead reports integrity_error

#### Scenario: App identity does not become administration
- **WHEN** an app-bound caller supplies a known policy UUID without an authorized readable association
- **THEN** lookup fails with access_denied without exposing the definition or retrying as a local administrator

### Requirement: Resources reference policies without a second configuration authority
Applications, sources, claims and policy-bearing operations SHALL associate definitions through UUID references. Embedded access/default/override bundles and inline policy configuration SHALL NOT be accepted as an alternative public contract. Resource inputs such as provider, selector, scope, subpath, platform, expected digest and destination SHALL retain their own meaning. Ordered release-filter references SHALL be distinct from unordered policy associations. Resolved effective values SHALL be reported as derived evidence, identifying their definition UUID/digest or built-in default; this evidence SHALL NOT be independently editable authority.

#### Scenario: Reject mixed configuration
- **WHEN** a request supplies policy references together with a former inline revision policy or claim override bundle
- **THEN** it fails with `invalid_request` before fetching, copying or mutation

#### Scenario: Inspect a chosen behavior
- **WHEN** a permitted request selects a referenced update policy and leaves revision history unspecified
- **THEN** its explanation identifies the update definition and the applicable revision default separately

### Requirement: Typed policies preserve independent behavior axes
Supported definitions SHALL cover access restrictions, source visibility, public claim scopes, retrieval cache mode, freshness, stale fallback, payload verification, path verification, ordered release asset filters and selection strategy, identical pulls, acquisition updates, revision history, reclaim recovery, usable-app protection and permitted policy values/bounds. Each definition SHALL apply only to its declared operation/resource target. Store integrity mode and optional credential verification SHALL remain independent of policy selection. Selecting no payload verification SHALL NOT disable metadata verification, safe path handling or archive structural checks. Acquisition update, revision history and retrieval freshness SHALL retain distinct effects.

#### Scenario: A history choice does not change fetching
- **WHEN** a claim adopts a different revision-policy UUID
- **THEN** its retrieval freshness and acquisition update interval remain unchanged
- **AND** any required initial check does not rewrite historical provenance or another scope's check state

#### Scenario: Irrelevant policy is supplied directly
- **WHEN** a temporary acquisition explicitly selects managed revision retention or usable-app protection
- **THEN** it fails before effects instead of ignoring the policy or publishing a managed output

### Requirement: Policy selection cannot broaden registered authority
All applicable registered access restrictions and policy constraints SHALL be evaluated separately from behavior selection. Configured allowance dimensions SHALL intersect, absent restrictions SHALL add no restriction and explicitly empty allowances SHALL permit nothing. A request-provided policy or catalog SHALL NOT grant authority, change discovery visibility, remove required protection or replace registered constraints. For new operations behavior precedence SHALL be explicit request, application default, source default, then built-in default. Existing claims SHALL retain their adopted behavior until explicit adoption; current access restrictions SHALL still govern every operation.

#### Scenario: Permissive request attempts an escape
- **WHEN** a restricted app supplies a permissive access-policy record in a caller catalog or selects no-store delivery
- **THEN** it cannot broaden the application's authority and the denied operation has no provider or output effects

#### Scenario: Defaults change after claim creation
- **WHEN** an application replaces its default update-policy reference
- **THEN** new defaulted claims use the new default while surviving claims retain their adopted behavior

### Requirement: Resolution is deterministic and rejects invalid associations before effects
Policy resolution SHALL use one coordinated metadata snapshot and bounded input. Unknown types or fields, duplicate UUIDs within an input list/catalog, wrong-target references, missing references, and multiple singleton definitions at the same precedence level SHALL fail before provider work or resource publication. Missing or corrupt catalog members SHALL produce `integrity_error`; an unknown caller reference SHALL produce `invalid_request`; incompatible valid policy choices SHALL produce `policy_conflict`. Arrays SHALL NOT imply precedence except the explicit ordered filter list. There SHALL be no recursive policy inheritance.

#### Scenario: Two update choices compete
- **WHEN** one request directly references two distinct update-policy definitions
- **THEN** it fails with `policy_conflict` regardless of their order or equal values

#### Scenario: Referenced persisted record is missing
- **WHEN** a verified catalog expects a policy member that is absent or fails integrity validation
- **THEN** resolution fails with `integrity_error` without substituting defaults or repairing the record

### Requirement: Caller catalogs preserve root-free fresh operations
Native and CLI operations SHALL support a caller-supplied catalog of standalone policy definitions independently of persistent registration. Inline temporary/no-store retrieval and temporary/caller-owned acquisition SHALL resolve those references without home lookup, a Saucepan store, app registration, credentials, keyring access or persistent writes. A caller catalog SHALL NOT shadow a persisted policy UUID. Persisted resources SHALL reference registered definitions only; managed publication with an unregistered definition SHALL fail preflight and SHALL NOT implicitly register it. Default operations SHALL not require a catalog or create synthetic policy identities.

#### Scenario: Fresh Git selection with a portable catalog
- **WHEN** an unregistered caller selects no-store retrieval and a caller-owned Git subpath copy using a sidecar policy catalog
- **THEN** fresh repository material and copied results are returned with their independent ownership
- **AND** existing caches remain unread and unchanged, and no source, index, policy or claim records are created

#### Scenario: Catalog shadows a registered definition
- **WHEN** a caller catalog repeats a UUID already registered in the selected store
- **THEN** validation fails before effects rather than replacing the registered definition

### Requirement: Adoption is atomic and fences in-flight operations
Policy association replacement SHALL be explicit and revision-checked. Application and claim changes SHALL check the application revision; source changes SHALL check the source attachment revision. Stale revisions SHALL fail even for an otherwise identical replacement. A successful claim adoption SHALL retain claim identity, context, selected output and active binding. Changes invalidating surviving claims SHALL fail atomically. Operations SHALL validate referenced definitions and relevant attachment revisions before effects and again at publication; unrelated catalog additions SHALL NOT alone invalidate their captured authority. Failed or superseded operations SHALL NOT advance successful update-check evidence.

#### Scenario: Adoption overtakes an update probe
- **WHEN** relevant policy associations change while an acquisition update prepares a candidate
- **THEN** publication reports a conflict without restoring old associations or changing the claim, binding or successful-check timestamp

#### Scenario: Tightening invalidates a survivor
- **WHEN** replacement constraints would disallow an existing claim's adopted policies
- **THEN** the replacement fails with `policy_conflict` and all associations remain unchanged

### Requirement: Historical policy evidence cannot grant current permission
Published outputs and completed checks SHALL preserve policy provenance and derived effective values needed to explain historical results and revision demand. Current resource associations SHALL remain the authority for new access and mutation. Changing policy references SHALL NOT rewrite immutable output provenance or inherit incompatible successful-check evidence. Removing an attachment SHALL NOT delete retained definitions, release live usage protection, or erase retention demand still carried by surviving records.

#### Scenario: Replay after permission narrows
- **WHEN** an app tries to reacquire a historical output after its current policy denies the source
- **THEN** the operation is denied even though the historical record shows a previously permitted acquisition

#### Scenario: A referenced definition outlives its application
- **WHEN** an application is uninstalled while retained output provenance refers to one of its former policies
- **THEN** that policy definition remains available for verified provenance and reclaim evaluation

### Requirement: Group policies narrow admission without granting authority
Group admission SHALL accept optional flat UUID references to access_policy and path-target verification_policy definitions. Each member SHALL first satisfy current caller and applicable registered resource restrictions, then every supplied group access predicate for action lease. Missing allowance dimensions SHALL impose no additional limit and explicitly empty allowances SHALL deny. Claim scope predicates SHALL use the actual selected claim scope; direct-output members SHALL NOT fabricate a scope and SHALL fail a scope-restricted group boundary. Group access predicates SHALL also constrain trusted local output groups. This group-only restrictive role SHALL NOT enable access-policy grants through ordinary acquisition requests or mutate registered associations.

#### Scenario: Unrestricted local group elects a boundary
- **WHEN** a local group boundary allows source A but a member belongs to source B
- **THEN** the group is denied despite the caller's unrestricted local access mode

#### Scenario: Request attempts to broaden registered permissions
- **WHEN** an app denied source B supplies a group policy allowing B
- **THEN** group admission is denied and the application's registered authority is unchanged

#### Scenario: Scope is not inferred from a label or another member
- **WHEN** a group restricts scopes but a member identifies only an output UUID
- **THEN** that member fails admission without borrowing another claim's scope or treating its name as a scope

### Requirement: Group policy resolution preserves typed verification and evidence
Group policy definitions SHALL remain immutable standalone records resolved through verified registered catalogs or validated caller catalogs under existing no-shadowing rules. Duplicate references, wrong policy types and unknown fields SHALL fail before admission effects. Multiple path-verification definitions SHALL conflict; access definitions SHALL intersect regardless of order. Group path verification SHALL be a floor over each member's existing requirement, and shared output verification SHALL satisfy the strongest applicable member requirement. Derived descriptors SHALL identify actual verification and applicable policy evidence without creating another editable policy authority. Corrupt expected catalog members SHALL fail integrity validation, while unknown caller references SHALL report invalid_request.

#### Scenario: Shared output needs full verification
- **WHEN** two members refer to one output and one member's resolved requirement is full while the group selects metadata
- **THEN** the shared output undergoes full verification before either member is returned

#### Scenario: Acquisition policy is used as a group policy
- **WHEN** group references include a revision, refresh, visibility or other non-group policy type
- **THEN** admission rejects the reference instead of ignoring it or performing another operation

#### Scenario: Portable boundary cannot shadow persisted authority
- **WHEN** a caller catalog duplicates a UUID already registered in the selected store
- **THEN** the group fails without replacing the definition or creating persistent policy records
