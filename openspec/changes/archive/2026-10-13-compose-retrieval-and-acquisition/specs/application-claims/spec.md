## MODIFIED Requirements

### Requirement: Ensuring a request does not duplicate a claim within its scope
Claim ensure SHALL require an explicit scope. Matching normalized requests with compatible effective policies in the same application and scope SHALL reuse a claim UUID, including concurrent requests. Different scopes SHALL retain independent claim identities and updates while allowing output reuse. Under output reuse and the default manual acquisition update policy, ensuring an available claim SHALL NOT implicitly refresh it. An explicitly configured due acquisition update SHALL be evaluated before reuse, following acquisition-refresh; it SHALL update the same claim only after permission checks and successful publication. Its check state and selection SHALL remain independent of other scopes. Explicit new-output policy SHALL acquire under normal source freshness rules but SHALL NOT create another claim identity; any resulting selection change SHALL use the existing claim and normal conflict checks. Identity lookup and final publication SHALL be coordinated with release, policy changes, context replacement, and uninstall.

#### Scenario: Repeated identical scoped acquisition
- **WHEN** concurrent callers ensure the same request, scope, policies, and context
- **THEN** they receive the same claim UUID and no duplicate association is persisted

#### Scenario: Projects update independently
- **WHEN** scopes A and B share output E1 and A updates its claim to E2
- **THEN** B still selects E1 through its own claim

## ADDED Requirements

### Requirement: Claims are optional composition without optional authorization
Retrieval and acquisition SHALL not require app registration unless the caller selects an app identity or a managed claim. App credentials SHALL remain independently opt-in. A persistent claim SHALL require a managed output, explicit app and scope, and permitted policies; temporary or caller-owned results SHALL not create claims. App-identified operations without claims SHALL still enforce registered source/action/scope restrictions, including an explicit authorization scope where required. Automatic update and revision-policy defaults/overrides SHALL be validated against app registration before effects and at commit. Omitted update policy SHALL retain manual behavior; omitted revision policy SHALL retain the existing latest-ten default.

#### Scenario: Acquire files without owning them as an app
- **WHEN** a local caller acquires a caller-owned directory without an app identity
- **THEN** no app registration, scope record or claim is synthesized

#### Scenario: App requests temporary files from a denied source
- **WHEN** an identified app selects temporary delivery for a source its policy denies
- **THEN** the operation fails before retrieval even though no claim would have been created

#### Scenario: Update policy is outside app allowance
- **WHEN** a claim request asks for automatic update frequency or revision retention outside its registered allowance
- **THEN** it reports policy_conflict before fetching or moving any selection
