## Purpose

Define the behavior-preserving reuse boundaries for source identity, Git checkout control flow, runtime naming, and integration-test fixtures, so implementation structure stays maintainable without changing Saucepan's observable behavior.

## Requirements

### Requirement: Native source-variant comparison
Saucepan SHALL use the Rust standard-library enum discriminant to distinguish index source variants and SHALL retain a separate origin comparison for entries of the same variant.

#### Scenario: Different source variants conflict
- **WHEN** an incoming sauce reuses an installed manifest name from another source variant
- **THEN** Saucepan returns the existing source-type conflict and preserves the installed entry without a custom source-variant matcher

#### Scenario: Same variant with another origin conflicts
- **WHEN** an incoming sauce reuses an installed manifest name from a different origin of the same source variant
- **THEN** Saucepan returns the existing origin conflict and preserves the installed entry

### Requirement: Single fresh-clone lifecycle
Saucepan SHALL use one clone and optional-ref-checkout path for both missing destinations and partial non-Git destinations while retaining cleanup only for the partial-destination case.

#### Scenario: Destination is missing
- **WHEN** a Git-backed sauce is fetched without an existing destination
- **THEN** Saucepan clones once and resolves the requested ref once when present

#### Scenario: Destination is partial
- **WHEN** a Git-backed sauce is fetched into an existing destination that is not a Git checkout
- **THEN** Saucepan removes the partial destination, then follows the same clone and optional-ref-checkout path

#### Scenario: Destination is a valid checkout
- **WHEN** a Git-backed sauce is fetched into a valid existing checkout
- **THEN** Saucepan preserves the existing no-ref pull behavior and requested-ref refresh behavior

### Requirement: Shared runtime naming rule
Saucepan SHALL define custom-Git terminal target-component extraction once in `src/utils/naming.rs` and SHALL use that rule for both update checkout selection and indexed artifact-path resolution.

#### Scenario: Custom-Git path is resolved by both consumers
- **WHEN** a custom-Git URL ends with a repository component
- **THEN** update and artifact-path resolution derive the same component before applying the existing repository-directory encoding

### Requirement: Context-named integration-test utilities
Saucepan's integration suite SHALL place reusable Git fixtures under `tests/utils/git.rs` and reusable index readers under `tests/utils/index.rs`. Git repository creation SHALL reuse the common test Git executor.

#### Scenario: Integration tests prepare Git state
- **WHEN** an integration scenario creates or mutates a Git repository
- **THEN** it uses the shared Git test utility rather than duplicating command environment setup

#### Scenario: Integration tests inspect the index
- **WHEN** an integration scenario reads index text or parsed JSON
- **THEN** it uses the shared index test utility while retaining behavior-specific assertions in the scenario
