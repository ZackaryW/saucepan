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

### Requirement: Persistent group policy roles remain distinct and restrictive
Persistent group definitions SHALL reference registered immutable policies by UUID. Group-level references SHALL accept only access restrictions and path-verification floors; acquisition behavior SHALL use each member's existing typed policy references and normal source/app defaults and constraints. Preparation SHALL intersect group restrictions with current authority for each action it actually requires, including acquire, update or recover; opening SHALL evaluate lease. A lease-only boundary SHALL NOT authorize preparation. Missing scope witnesses SHALL fail scope-restricted boundaries. New definitions SHALL require new policy UUIDs and explicit association adoption. Group replacement SHALL NOT silently adopt policies or replace context on an existing claim. Portable caller catalogs SHALL remain available for inline leases but SHALL NOT become unregistered authorities for persisted groups.

#### Scenario: Lease boundary does not permit fetching
- **WHEN** a group allows lease but needs an acquire action to prepare missing content
- **THEN** preparation is denied before provider work

#### Scenario: Member behavior does not become group authority
- **WHEN** a member's permissive behavior references conflict with current app source restrictions
- **THEN** preparation is denied and no group reference broadens the app's authority

#### Scenario: Existing claim has another adopted behavior
- **WHEN** group replacement supplies different update or revision behavior for a matching existing claim
- **THEN** preparation requires explicit permitted claim adoption instead of mutating it through group registration

### Requirement: Durable pin policy is an explicit separate retention role
A pin request SHALL reference exactly one registered immutable pin_policy with retention until_released. It SHALL reject other policy roles, unknown fields and conflicting references. Pin policy SHALL not be inherited through source/application defaults or ordinary claim/group references. Ordinary acquisition and inline group admission SHALL reject pin policy in their existing roles. Group preparation SHALL opt in through a separate pin request, not an embedded policy definition. Current authority SHALL support distinct pin and unpin actions; selecting pin policy SHALL NOT grant either permission or disable required verification. Removing a policy attachment or changing current permissions SHALL not retroactively release already committed pins.

#### Scenario: Pin behavior is not silently inherited
- **WHEN** a caller prepares a group without a pin request
- **THEN** no durable pin is created even when the catalog contains pin_policy definitions

#### Scenario: A lease-only caller requests durable retention
- **WHEN** an app allowed lease but denied pin attempts pin creation
- **THEN** the operation fails despite a valid pin policy reference

### Requirement: Effective policy explanation is authorized and read-only
Native and CLI interfaces SHALL explain effective policies for typed retrieval, acquisition, composed and existing-claim inputs without provider work, resource publication, probing, registration or adoption. Applicable axes SHALL report effective values, origin level and definition UUID/digest or built-in default, plus applicable restriction and constraint evidence. Inapplicable axes SHALL be distinguished from executed settings. Existing claim explanations SHALL distinguish adopted behavior from current restrictions. Explanations SHALL use the same resolution rules as execution and a coordinated verified snapshot for persisted inputs. They SHALL NOT become editable authority or a guarantee of future authorization, content availability or successful execution. Ordinary store-open recovery SHALL retain its current contract.

#### Scenario: Explain a customized recipe
- **WHEN** an authorized caller explains a composed request with profile-expanded refs and an explicit latest-three policy
- **THEN** the result identifies its actual leaf UUIDs, effective revision count and applicable default verification separately without contacting the provider

#### Scenario: Inspect a surviving claim after app defaults change
- **WHEN** an authorized caller explains a claim whose app defaults have since changed
- **THEN** the explanation retains the claim's adopted behavior and separately reports current restrictions rather than adopting the new defaults

### Requirement: Explanation preserves portable and scoped boundaries
Inline portable explanations SHALL not require a root, home, registration or keyring. Persisted explanations SHALL enforce current read authorization for every referenced resource and applicable credentials without exposing protected definitions through a denial. Unknown, wrong-target or conflicting references SHALL produce the existing structured validation errors; missing or corrupt expected records SHALL produce integrity_error. Current restrictions SHALL be re-evaluated by a later actual operation regardless of an earlier explanation.

#### Scenario: Denied app attempts to inspect another scope
- **WHEN** an app-bound explanation requests a claim outside its permitted scope
- **THEN** it returns access_denied with no protected explanation payload and no unbound retry

#### Scenario: Root-free temporary preview
- **WHEN** a caller explains an inline temporary retrieval using only its portable catalog
- **THEN** it receives effective settings without looking up or creating a store
