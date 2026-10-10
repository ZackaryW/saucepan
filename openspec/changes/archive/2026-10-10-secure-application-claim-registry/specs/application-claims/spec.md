# Spec Delta

## MODIFIED Requirements

### Requirement: Applications claim specific outputs and retain their requests
Registered applications SHALL have UUID-indexed records with settings and separate application-owned claim registries. Each claim SHALL retain a claim identity, the acquisition request including item policies, its selected output UUID, an explicit nonempty scope of at most 4096 UTF-8 bytes, and a plain string-to-string context map. Scope SHALL be compared literally without inferred filesystem normalization. Omitted context SHALL mean an empty map in the current contract. Successful scoped acquisition SHALL create or update the intended claim; failed acquisition SHALL leave existing claims unchanged. Supplied acquisition context SHALL be published atomically with a new claim. Claims SHALL represent usage associations rather than exclusive resource ownership or authentication proof. Context SHALL NOT participate in source identity, output identity, item grouping, output deduplication, or retention policy. Explicit scope, rather than context, SHALL determine independent claim ownership. Claim ensure and reuse SHALL follow the requirements below.

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

### Requirement: App views are scoped to claims
After applying its opt-in credential requirement, an app view SHALL contain its registry claims and their referenced metadata, subject to its configured source/claim access restrictions and view filters. A matching source cached only by another app SHALL NOT appear as a claim. Each view SHALL report its explicit scope. Listing SHALL NOT create claims, and view filters SHALL NOT act as acquisition permissions. Source and claim restrictions SHALL be evaluated separately before acquisition, reuse, access or mutation; disabling credentials SHALL NOT bypass those restrictions.

#### Scenario: Another application caches a matching resource
- **WHEN** app A lists its view after B acquires an otherwise matching item
- **THEN** A has no claim to that item unless A actually acquires or explicitly claims it

### Requirement: Claim context is explicit application-owned metadata
Context keys SHALL be nonempty strings and values SHALL be strings, including empty strings. Keys and values SHALL be case-sensitive and preserved without path, whitespace, or Unicode normalization. Dots in keys SHALL be literal characters, not nested field paths. Context SHALL permit at most 64 entries, keys of at most 128 UTF-8 bytes, and values of at most 4096 UTF-8 bytes. Invalid context SHALL fail before source acquisition or claim mutation. Identical context on distinct claims SHALL be permitted and SHALL NOT imply uniqueness, ownership, or automatic future acquisition. An ensure that reuses a claim SHALL preserve identical supplied context and reject differing supplied context with `claim_context_conflict`, without overwriting labels or creating a second claim. Explicit whole-map context replacement remains the mutation path.

#### Scenario: Context remains plain data
- **WHEN** a caller supplies a nested object or number as a context value, an empty key, or an over-limit map
- **THEN** the operation fails without acquiring content or changing claims

#### Scenario: Literal values are preserved
- **WHEN** context contains a Windows path, a dotted key, or a value containing `*`
- **THEN** the stored context preserves those strings literally and does not interpret them as queries

## ADDED Requirements

### Requirement: Application registration has explicit stable identity
Registration SHALL accept a caller-owned registration key distinct from the display name. Concurrent or repeated registration with the same key and equivalent settings SHALL identify one application. Different settings SHALL conflict until explicitly changed with revision checking. Knowing the key SHALL NOT grant protected claim access or cause credentials to be reissued. Different registration keys SHALL be permitted to share a display name.

#### Scenario: Two clients register one integration
- **WHEN** clients concurrently register the same application key and settings
- **THEN** one live registration exists and both authorized results identify it
- **AND** a retry cannot retrieve a previously issued secret

### Requirement: Ensuring a request does not duplicate a claim within its scope
Claim ensure SHALL require an explicit scope. Matching normalized requests with compatible effective policies in the same application and scope SHALL reuse a claim UUID, including concurrent requests. Different scopes SHALL retain independent claim identities and updates while allowing output reuse. Under output reuse policy, ensuring an available claim SHALL NOT implicitly refresh it. Explicit new-output policy SHALL acquire under normal source freshness rules but SHALL NOT create another claim identity; any resulting selection change SHALL use the existing claim and normal conflict checks. Identity lookup and final publication SHALL be coordinated with release, policy changes, context replacement, and uninstall.

#### Scenario: Repeated identical scoped acquisition
- **WHEN** concurrent callers ensure the same request, scope, policies, and context
- **THEN** they receive the same claim UUID and no duplicate association is persisted

#### Scenario: Projects update independently
- **WHEN** scopes A and B share output E1 and A updates its claim to E2
- **THEN** B still selects E1 through its own claim

### Requirement: Covered folder requests reuse an existing claim
A compatible natural-directory or Git folder request fully covered by an existing same-scope claim SHALL return that claim and an explicit relative view, without a new claim record. Coverage SHALL require matching source and selector semantics, compatible effective policies, immutable provenance and manifest evidence, and safe path-component containment. Exact matches SHALL take precedence over covering matches; otherwise the most specific covering folder SHALL win deterministically. Raw/extracted format changes and different binary/platform selection SHALL NOT be treated as folder coverage. Returned views SHALL use the owning claim's context.

A broader request that would subsume an existing narrower claim, or a same-scope overlap with incompatible policies, SHALL fail with `claim_overlap` or `policy_conflict` without publishing a redundant claim or silently changing existing identities. Query globs SHALL NOT create ownership coverage.

#### Scenario: Request a folder already owned by a repository claim
- **WHEN** a scope already claims a compatible repository export and ensures its available `skills/core` folder
- **THEN** the result identifies the existing claim, its selected output, and the `skills/core` view
- **AND** the registry still contains one claim

#### Scenario: A parent request follows a child claim
- **WHEN** a scope already claims `skills/core` and requests a broader export covering it
- **THEN** ensure reports `claim_overlap` and leaves the narrower claim unchanged

#### Scenario: Similar path prefixes are not containment
- **WHEN** a claim covers `skills/core` and a request selects `skills/core-extra`
- **THEN** the request is not treated as covered by that claim

#### Scenario: Labels differ on a matching request
- **WHEN** a matching request is ensured with different supplied context
- **THEN** `claim_context_conflict` is returned without duplicate claims or label changes
