## ADDED Requirements

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
