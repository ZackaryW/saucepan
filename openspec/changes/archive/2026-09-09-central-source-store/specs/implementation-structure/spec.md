## REMOVED Requirements

### Requirement: Native source-variant comparison
**Reason**: The rewrite defines canonical provider/source identity rather than prescribing the legacy enum-comparison implementation.
**Migration**: Compare the canonical source identity used by the new acquisition model.

### Requirement: Single fresh-clone lifecycle
**Reason**: Workspace checkout replacement is superseded by central source reuse; unrecognized directories must not be silently removed to satisfy a clone request.
**Migration**: Use the central acquisition flow and stage owned source preparation.

### Requirement: Shared runtime naming rule
**Reason**: Source and content paths are derived from canonical identities, not a shared legacy terminal-component naming helper.
**Migration**: Resolve content through the core's recorded artifact paths.

### Requirement: Context-named integration-test utilities
**Reason**: Prescribed legacy helper paths are not product behavior and do not justify maintaining a second implementation's test layout.
**Migration**: Keep reusable fixtures appropriate to the new core and verify its observable contracts.

## ADDED Requirements

### Requirement: Rejected designs remain excluded under every name
Planning, implementation, tests, and completion claims SHALL preserve the rejected designs and exclusions recorded in proposal.md. A renamed helper, safety feature, dependency, compatibility requirement, or new task SHALL NOT reintroduce the same excluded behavior. Historical source, tests, schemas, canonical legacy specs, commits, memory entries, and assistant claims SHALL NOT override a user correction. Reopening a rejected boundary SHALL require an explicit user decision changing that boundary; generic continuation instructions SHALL retain the accepted scope. Unresolved design questions SHALL remain identified as unresolved rather than silently becoming requirements. These rules SHALL NOT add permission steps to routine work within the accepted scope.

#### Scenario: Rejected behavior gets a new name
- **WHEN** a proposed safety helper adds dependency grants, or a proposed acquisition unit gives folders separate snapshot histories
- **THEN** it is excluded as the same rejected behavior and is not added as a new requirement or implementation task

#### Scenario: Old evidence conflicts with a correction
- **WHEN** an old test, source module, commit lesson, or assistant statement requires an excluded subsystem
- **THEN** the revised user boundary governs and the old evidence does not justify restoring that subsystem

#### Scenario: User asks to continue
- **WHEN** the user says next, proceed, continue automatically, or finish implementation without revising a rejected decision
- **THEN** work continues within the accepted scope and excluded behavior remains excluded

#### Scenario: Settled decision is presented as open again
- **WHEN** implementation proposes reconstructing unsaved history or replacing caller tokens after ordinary index changes
- **THEN** it follows the settled gap-preserving retention and stable-token contracts instead of treating the rejected alternative as new necessary work

### Requirement: New implementation follows the supplied structural boundaries
The application SHALL be implemented from the ground up in a new `src/`, following `src-struct`: CLI adapters call the core; models describe data; policies evaluate explicit inputs; sources acquire content; utils contains domain-neutral helpers. The earlier legacy source in `src2/` and rejected implementation in `src3/` SHALL remain ignored and outside root build/package dependencies. The new app SHALL NOT copy, wrap, import, include, dispatch to, or incrementally port either implementation. Neither reference requires a maintained manifest or independently runnable tests.

#### Scenario: Build the new implementation
- **WHEN** the root package is built or packaged
- **THEN** it uses only the fresh source tree without compiling, depending on, or packaging src2 or src3

#### Scenario: Begin the replacement
- **WHEN** implementation starts after the source rename
- **THEN** it creates new src targets against the revised contracts rather than repointing Cargo to src3 or restoring the rejected service behind adapters

### Requirement: Acceptance is based on newly implemented behavior
Tests for the replacement SHALL be authored against the revised capability contracts. Passing tests, public APIs, schemas, and completion claims from either prior implementation SHALL NOT impose compatibility requirements or establish completion for the new app. Build configuration SHALL include only dependencies and targets justified by the fresh implementation.

#### Scenario: Old test expects an excluded subsystem
- **WHEN** an old test requires permission administration, runtime activation, or another excluded behavior
- **THEN** the replacement's tests follow the revised scope instead of recreating the subsystem to satisfy that test

#### Scenario: Previous progress exists
- **WHEN** a capability had passing tests or a completed task in the rejected implementation
- **THEN** its replacement remains unverified until newly written tests demonstrate the fresh implementation's required behavior

### Requirement: Generic utilities are implemented first using red-first tests
The fresh implementation SHALL begin with domain-neutral helpers under src/utils following src-struct. Each helper SHALL have behavioral tests observed failing before its implementation, followed by passing tests. Helpers SHALL use concise, reusable interfaces appropriate to their inputs and SHALL NOT own app settings, index schemas, source identity policy, or permission rules. A library entry for testing utilities SHALL NOT imply a completed CLI or justify a placeholder application.

#### Scenario: Utility helper implementation
- **WHEN** a new generic helper is implemented
- **THEN** its completion evidence includes the initial failing behavior tests and subsequent passing tests, with domain rules left to the core

### Requirement: Native and CLI entry points share the same core behavior
Native API and CLI calls SHALL use the same acquisition, index, policy, and app sub-index verification behavior. Scopes SHALL describe the selection used to form a sub-index, and authoritative mode SHALL authenticate that app view. Models, adapters, and tests SHALL NOT translate scopes into a resource-grant hierarchy, dependency allowlist, role/action/destination permission framework, or runtime-management subsystem.

App registration/configuration, touched-entry bookkeeping, and initial filters SHALL be handled by the same core for both adapters. Settings SHALL be persisted in and read from the central encrypted app record; neither adapter SHALL load legacy TOML app settings. The sub-index SHALL be a scoped view of that core's central index. Neither adapter SHALL require a separate sub-index issuance/refresh service.

#### Scenario: Equivalent requests through two adapters
- **WHEN** equivalent recipe or app sub-index verification requests are made through the native API and CLI
- **THEN** they observe the same source, cache, policy, and verification outcomes

### Requirement: Supported OS adapters preserve shared behavior
Windows, macOS, and Linux implementations SHALL expose the same logical app registration/configuration, acquisition, scoped-index, and verification behavior. Platform adapters SHALL handle native secret access, home paths, file locking/replacement, and representable filesystem entries without redefining core policy or cryptographic formats. Identical canonical payloads and test keys SHALL yield identical HMAC results on each platform. Verification evidence SHALL distinguish shared-core tests, native-platform tests, and untested environments; previous implementation tests SHALL NOT establish current platform support.

#### Scenario: Cross-platform core vectors
- **WHEN** the same canonical source inputs, authenticated payload, and explicit test key are evaluated on supported platforms
- **THEN** the SHA-256/HMAC results and app-visible behavior agree

#### Scenario: Platform publication fails
- **WHEN** native locking or file replacement cannot complete
- **THEN** the operation reports failure and preserves committed state without applying a weaker platform-specific index policy
