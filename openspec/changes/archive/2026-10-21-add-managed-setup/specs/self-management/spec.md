# Spec Delta

## MODIFIED Requirements

### Requirement: First setup is explicit and supports profile-prepared release requests
Saucepan SHALL document and support first setup beginning with an explicitly obtained release manager and matching built shim. The low-level flow SHALL explicitly initialize the selected root, register ordinary profile-expanded installation policies and GitHub release source/filter records, and invoke the existing bootstrap operation. A higher-level `setup` operation SHALL perform that sequence from an explicit exact release tag and matching built shim, preserving the existing managed publication contract. Profile choice SHALL not clone a repository, choose a different identity/scope contract, alter verification modes or replace normal platform selection. The default root SHALL retain existing user-home .saucepan resolution. Setup instructions SHALL describe adding its bin directory to PATH without editing shell profiles or environment settings automatically. Installing a language SDK SHALL NOT perform these steps.

#### Scenario: Basic setup without a keyring
- **WHEN** a caller explicitly initializes a basic root and bootstraps from a GitHub release request with registered explicit-update installation refs
- **THEN** the existing release acquisition, version probe, managed claim and stable shim flow succeeds without a keyring requirement

#### Scenario: SDK package installed without a manager
- **WHEN** a user installs or imports a language SDK with no manager present
- **THEN** no Saucepan root, binary, source, claim or launcher is created and attempted manager operations report the missing executable

#### Scenario: One setup invocation
- **WHEN** a caller invokes setup with an exact supported release tag and matching shim against a fresh root
- **THEN** Saucepan initializes the root, prepares the ordinary installation inputs and reports completion only after the managed launcher, claim and verified binding are published
- **AND** the selected output matches that tag even if a newer release exists

## ADDED Requirements

### Requirement: Setup reuses a verified installation without implicit update
Setup SHALL reuse an intact self-managed installation at the requested tag without network discovery, new registrations, launcher replacement or binding mutation. A different installed version SHALL produce a diagnostic directing explicit self-update, without upgrading or downgrading. Existing pending operations SHALL be identified rather than bypassed with a new application. Unknown occupants, damaged owned launchers and ambiguous self identities SHALL not be treated as an empty destination. Setup SHALL be trusted local administration and SHALL preserve explicit root and credential boundaries.

#### Scenario: Repeat completed setup
- **WHEN** setup is invoked again for the already installed tag with an intact managed installation
- **THEN** it returns the existing app, claim, output and launcher with `changed` false and performs no release acquisition

#### Scenario: Another version is installed
- **WHEN** setup requests a different tag from a ready shared installation
- **THEN** the existing installation remains unchanged and the result explains the explicit update route

#### Scenario: App-bound caller
- **WHEN** an application-bound context requests administrative setup
- **THEN** it is rejected without dropping identity or falling back to unrestricted execution

### Requirement: Setup status reports current ownership and readiness
`setup-status` SHALL provide versioned JSON identifying the canonical root, state, applicable installation identities, launcher, installed version, pending operation and diagnostics. States SHALL distinguish absent, unmanaged, ready, pending and conflict. Ready SHALL require current integrity-verified application, claim, active binding and owned launcher evidence; a version response, filename or historical success receipt alone SHALL not establish it. Inspection SHALL not initialize, fetch, probe candidates, resume setup, repair or alter PATH. A missing root SHALL be observable as absent without creation; corrupt or unsupported stores SHALL remain errors. Successful inspection SHALL exit zero independently of readiness.

#### Scenario: Inspect a missing installation
- **WHEN** setup-status runs against an absent root
- **THEN** it returns absent without creating the root or contacting a release service

#### Scenario: Raw manager occupies the launcher path
- **WHEN** an unowned raw manager exists at the intended launcher path
- **THEN** inspection reports unmanaged rather than ready

#### Scenario: Historical setup completed but launcher changed
- **WHEN** a completed setup receipt exists but current launcher bytes or ownership no longer match
- **THEN** inspection reports the observed problem without treating the old receipt as current readiness

### Requirement: Explicit self-update can select an exact release tag
Self-update SHALL accept an optional exact tag. Omission SHALL retain current stored-selector behavior. An explicit tag SHALL select a replacement release for the same self application and claim while preserving source, scope, platform and adopted policy references. The new request, selected output and active binding SHALL commit together only after existing candidate validation and revision checks. Failure SHALL preserve the prior request, claim selection, binding and shim. The tag SHALL participate in retry identity; no setup or ordinary lookup SHALL implicitly invoke this update.

#### Scenario: Advance a pinned installation
- **WHEN** an explicit self-update selects a different valid tag for a setup-created pinned claim
- **THEN** successful publication updates the request and claim/binding together without replacing the shim

#### Scenario: Replacement tag fails validation
- **WHEN** the requested release cannot pass candidate validation
- **THEN** the original request and active selection remain usable and the failure is inspectable
