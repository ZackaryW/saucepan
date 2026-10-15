# Spec Delta

## Purpose

Provide optional named starting points for resource behavior while keeping flat immutable policy definitions and explicit associations as Saucepan's sole policy authority.

## ADDED Requirements

### Requirement: Named profiles expand without operational side effects
Saucepan SHALL expose native and CLI listing and expansion of `managed`, `explicit-update` and `temporary` profiles. Listing and expansion SHALL require neither a store nor home discovery and SHALL NOT fetch, initialize, register, claim, pin, probe or contact a keyring. An expansion SHALL return its name, recipe digest, ordinary flat policy records and separate retrieval, acquisition and combined UUID references. Unknown names SHALL fail with invalid_request. Omitting the name from an explicit expansion SHALL select managed. Existing operations that do not request expansion SHALL retain ordinary defaults without synthetic policy identities or registration. Profiles SHALL NOT be accepted as a new policy definition type or persisted profile inheritance graph.

#### Scenario: Preview without a home or store
- **WHEN** a caller lists profiles and expands temporary without a configured home, root or application
- **THEN** it receives the recipe and target-specific references without filesystem, provider or registration effects

#### Scenario: Existing caller omits profiles
- **WHEN** an existing resource request has no profile expansion or policy references
- **THEN** normal built-in defaults apply without registering profile records

### Requirement: Built-in recipes preserve independent operation behavior
Managed SHALL select cached retrieval, max-age freshness of 86,400 seconds, no stale fallback, manual acquisition updates, identical-content reuse, latest-ten managed revision history and report-unavailable recovery. Explicit-update SHALL select the same policies except retrieval freshness SHALL be always. Neither profile SHALL schedule work, refresh on lookup or automatically activate a new executable. Explicit-update SHALL check origin material when retrieval is explicitly requested while preserving unchanged-content reuse. Temporary SHALL select temporary retrieval, always freshness and no stale fallback; it SHALL omit managed revision, protection, update and reclaim settings. Acquisition destination SHALL remain an explicit resource input. Temporary delivery to a managed output SHALL fail preflight.

#### Scenario: Managed cache within freshness window
- **WHEN** a managed-profile retrieval has a valid cached result checked less than 24 hours ago
- **THEN** retrieval reuses it without an origin check and no acquisition update is scheduled

#### Scenario: Explicit update versus read-only use
- **WHEN** an explicit-update consumer looks up its existing selected output and later explicitly retrieves the source
- **THEN** lookup performs no fetch and the retrieval checks the origin even within the ordinary 24-hour window
- **AND** identical material does not force a new revision

#### Scenario: Temporary operation has an explicit lifetime
- **WHEN** a caller selects temporary retrieval and temporary acquisition through guarded execution
- **THEN** material is fetched fresh, available for the guarded lifetime and cleaned according to native ownership without persistent managed cache entries
- **AND** a separate caller-owned destination can preserve copied results without converting them to managed history

### Requirement: Recipe identities and adoption remain immutable
Equivalent built-in leaf definitions SHALL retain the same UUID across repeated expansions and recipes. A changed definition SHALL use a new UUID and a changed recipe SHALL have a different digest. Registering expansion records SHALL use normal immutable registration and SHALL NOT attach them to a resource. Existing claims SHALL keep adopted behavior until explicit authorized revision-checked adoption. A later recipe with the same name SHALL NOT reinterpret persisted UUID references or historical evidence. Built-in identity conflicts SHALL fail rather than overwrite persisted content.

#### Scenario: Manager introduces a different history recipe
- **WHEN** a later managed recipe references a new revision-policy UUID and an existing claim still references the old one
- **THEN** expanding or registering the new recipe leaves the claim unchanged until explicit adoption

#### Scenario: Repeated expansion and registration
- **WHEN** an unchanged recipe is expanded and its records registered twice
- **THEN** both expansions contain the same leaf identities and the second registration is idempotent

### Requirement: Profile customization produces ordinary validated references
Expansion SHALL accept bounded flat caller catalogs and separate retrieval/acquisition override references. One explicit singleton override SHALL replace the recipe choice on that axis before ordinary evaluation. Duplicate references, missing definitions, wrong-target choices and two explicit singleton choices for one axis SHALL fail before effects; array order SHALL NOT resolve conflicts. The combined reference result SHALL contain a representable non-conflicting union. Customization SHALL NOT alter definitions or create a new precedence level. Expanded request associations SHALL retain normal request/application/source/built-in precedence and current constraints. Persisted operations SHALL explicitly register definitions first; portable operations SHALL retain existing root-free catalog semantics and no-shadowing rules.

#### Scenario: Customize managed history
- **WHEN** a caller expands managed with one acquisition override referencing a flat latest-three definition
- **THEN** the returned acquisition and combined refs replace latest-ten with latest-three while retrieval refs remain unchanged
- **AND** a registered minimum-five constraint still rejects the resulting acquisition

#### Scenario: Two custom choices conflict
- **WHEN** a caller supplies two distinct revision-policy overrides for the same expansion
- **THEN** expansion reports policy_conflict regardless of order or equal contents

### Requirement: Profiles do not imply authority or lifetime protection
Profiles SHALL NOT enable credentials, change store integrity, configure visibility, invent scope, grant access, select verification overrides or create durable pins. Their omitted axes SHALL resolve through normal applicable policies and built-in behavior, including existing verification checks. Latest-ten history SHALL retain its existing committed-order window and maximum surviving demand semantics; independent pins, leases, active bindings and pending lifecycle intent SHALL remain additional protection. No profile SHALL release such protection or make a bare path a retained handle.

#### Scenario: Scoped denial survives profile choice
- **WHEN** a credential-free app restricted to source A requests source B with any profile
- **THEN** current access rules deny it without administrative fallback

#### Scenario: Old pinned revision exceeds the window
- **WHEN** a managed-profile item's pinned old output falls outside its latest-ten window
- **THEN** it remains protected in addition to the normal window until explicit release and all other protections permit reclaim

### Requirement: Installation preparation is explicit and uses normal leaves
Profile expansion SHALL support an explicit for-install option for managed and explicit-update that includes an ordinary useable_app_policy in the acquisition and combined refs. This option SHALL have no installation or registration effects and SHALL report the added protection choice. Temporary with for-install SHALL fail with policy_conflict. Source, platform filters, executable path, probe, scope, built shim and destination SHALL remain lifecycle/resource inputs rather than profile fields.

#### Scenario: Prepare policies for Saucepan setup
- **WHEN** a caller expands explicit-update for installation
- **THEN** the result contains the ordinary recipe leaves and usable-app protection with no source registration, claim, shim or installation yet created

#### Scenario: Reject a temporary stable installation
- **WHEN** a caller requests temporary for installation
- **THEN** expansion fails without creating persistent or temporary content
