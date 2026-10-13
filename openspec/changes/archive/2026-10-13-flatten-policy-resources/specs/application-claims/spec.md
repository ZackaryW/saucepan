## MODIFIED Requirements

### Requirement: Applications claim specific outputs and retain their requests
Registered applications SHALL have UUID-indexed records and separate application-owned claim registries. Application defaults and constraints SHALL be attached as policy UUID references. Each claim SHALL retain a claim identity, its resource request and adopted policy references, its selected output UUID, an explicit nonempty scope of at most 4096 UTF-8 bytes, and a plain string-to-string context map. Derived effective-policy evidence SHALL NOT constitute a second editable policy authority. Scope SHALL be compared literally without inferred filesystem normalization. Omitted context SHALL mean an empty map. Successful scoped acquisition SHALL create or update the intended claim; failed acquisition SHALL leave existing claims unchanged. Supplied acquisition context SHALL be published atomically with a new claim. Claims SHALL represent usage associations rather than exclusive resource ownership or authentication proof. Context SHALL NOT participate in source identity, output identity, item grouping, output deduplication, or retention policy. Explicit scope, rather than context or policy UUID, SHALL determine independent claim ownership. Claim ensure and reuse SHALL follow the requirements below.

#### Scenario: An application claims two selections from one repository
- **WHEN** it acquires two disjoint folders or different platform selections
- **THEN** distinct claims preserve the corresponding requests and selected entries without collapsing them into a source-only claim

#### Scenario: Two project contexts share content
- **WHEN** an application acquires identical content with reuse policy under two different explicit scopes and project context values
- **THEN** each scope has its own claim carrying the supplied context while both can select the same output
- **AND** neither context creates a project directory in the store

#### Scenario: Acquisition fails with supplied context
- **WHEN** a context-bearing acquisition fails before claim publication
- **THEN** no partial claim or context-only association is published

### Requirement: Ensuring a request does not duplicate a claim within its scope
Claim ensure SHALL require an explicit scope. Matching normalized requests with compatible effective policies in the same application and scope SHALL reuse a claim UUID, including concurrent requests. Equivalent effective choices under different policy UUIDs SHALL NOT alone create duplicate claims. Reuse SHALL NOT silently adopt the new references; explicit revision-checked adoption SHALL retain claim identity and context. Different scopes SHALL retain independent claim identities and updates while allowing output reuse. Under output reuse and the default manual acquisition update policy, ensuring an available claim SHALL NOT implicitly refresh it. An explicitly configured due acquisition update SHALL be evaluated before reuse, following acquisition-refresh; it SHALL update the same claim only after permission checks and successful publication. Its check state and selection SHALL remain independent of other scopes. Explicit new-output policy SHALL acquire under normal source freshness rules but SHALL NOT create another claim identity; any resulting selection change SHALL use the existing claim and normal conflict checks. Identity lookup and final publication SHALL be coordinated with release, policy adoption, context replacement, and uninstall.

#### Scenario: Repeated identical scoped acquisition
- **WHEN** concurrent callers ensure the same request, scope, policies, and context
- **THEN** they receive the same claim UUID and no duplicate association is persisted

#### Scenario: Projects update independently
- **WHEN** scopes A and B share output E1 and A updates its claim to E2
- **THEN** B still selects E1 through its own claim

#### Scenario: Equivalent policy definitions do not duplicate ownership
- **WHEN** a same-scope ensure selects equivalent behavior using another registered policy UUID
- **THEN** it reuses the compatible claim without changing its adopted references
- **AND** an incompatible overlapping policy choice fails without creating another claim

## ADDED Requirements

### Requirement: Optional claims adopt resolved behavior without embedded overrides
A managed claim SHALL adopt the resolved behavior references or built-in choices used at creation. Subsequent changes SHALL require explicit revision-checked adoption under current registered constraints. Temporary and caller-owned acquisitions SHALL NOT create claims. App-identified operations without a claim SHALL still apply source/action/scope restrictions and policy constraints. Covered folder views SHALL retain the owning claim and its adopted policies; policy references SHALL NOT turn query globs into ownership coverage.

#### Scenario: Unclaimed app operation remains restricted
- **WHEN** an identified app requests temporary files from a denied source with permissive behavior references
- **THEN** it fails before retrieval even though no claim would be created

#### Scenario: Covered folder keeps its owner
- **WHEN** a compatible subfolder is already covered by a same-scope claim
- **THEN** the returned relative view uses that claim and its adopted policies without a new association
