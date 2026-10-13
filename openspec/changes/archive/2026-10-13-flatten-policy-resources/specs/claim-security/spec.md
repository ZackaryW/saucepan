## MODIFIED Requirements

### Requirement: One operation policy governs application access
App-facing lookup, retrieval, acquisition, claim ensure/reuse, leasing, recovery, update, release, activation and removal SHALL evaluate caller, action and resource against verified referenced access policies and constraints. Omitted restrictions SHALL permit supported actions within the app ownership boundary; an explicitly empty allowance SHALL permit none. Configured dimensions and applicable restriction records SHALL intersect, and a permitted source SHALL NOT override a denied action or scope. Cache hits, covered views, historical effective-policy evidence and direct UUID lookup SHALL NOT bypass current policy. Presentation filters SHALL remain distinct from authorization. Restricted app calls SHALL NOT broaden their registration, attach authoritative request-supplied policies or select trusted administrative mode. Authoritative policy association changes SHALL use explicit local store administration with revision checking and applicable proof. Credential enforcement and source/claim visibility SHALL remain independent opt-ins. Native, CLI, maintenance and shim paths SHALL preserve their existing authority boundaries while using the same resolved policy meaning.

#### Scenario: A cached output would bypass acquisition restrictions
- **WHEN** an app requests an already-cached output belonging to a forbidden source
- **THEN** app-scoped access is denied even if no download is required

#### Scenario: An empty scope allowance is configured
- **WHEN** an app has an explicitly empty allowed-scope set
- **THEN** it cannot create or access claims in arbitrary scopes

#### Scenario: An app attempts to broaden its own policy
- **WHEN** a restricted app operation attempts to modify registered source allowances
- **THEN** it is denied without altering the registration

#### Scenario: Request policy cannot grant authority
- **WHEN** a restricted caller supplies a permissive access definition or tries to replace registered constraints in an acquisition request
- **THEN** the operation is rejected before effects and its registered authority remains unchanged

#### Scenario: Credential-free policy adoption is still revision checked
- **WHEN** an administrator replaces policy associations for an app whose credentials are disabled using a stale expected revision
- **THEN** the change conflicts without asking for an unnecessary token or modifying associations
