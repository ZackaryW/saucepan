# self-management Specification

## Purpose

Keep managed executables usable through stable shims and policy-governed activation, including Saucepan acquiring and updating its own release assets.

## Requirements

### Requirement: Stable shims resolve explicit active bindings
A managed application's stable shim SHALL resolve an active output UUID and executable-relative path from its application metadata without requiring the target executable to perform that lookup. It SHALL forward arguments, standard streams, exit status, and supported process interruption behavior. A newer download SHALL NOT implicitly become active.

#### Scenario: A newer release is downloaded but not activated
- **WHEN** the shim launches while an older entry remains the committed active binding
- **THEN** it executes the older entry rather than the newest downloaded one

### Requirement: The useable_app policy preserves a valid active target
The policy identifier SHALL be `useable_app`. It SHALL protect the active executable output from reclaim and retain the previous target through candidate validation. Activation SHALL require complete content, a matching platform, a valid executable path, and a successful bounded usability probe. The binding switch SHALL be atomic and coordinated with execution leases and reclaim. Failed activation SHALL preserve the previous usable binding.

#### Scenario: Candidate validation fails
- **WHEN** a downloaded executable fails its configured usability probe
- **THEN** the active binding remains on the previous entry and that entry remains protected

#### Scenario: Activation succeeds while the old process is running
- **WHEN** a valid candidate is activated and the old entry has a live execution lease
- **THEN** new shim launches use the candidate and the old process retains its content until exit
- **AND** the old entry becomes eligible for ordinary retention after its protection and leases end

### Requirement: Saucepan self-management uses release assets
Saucepan SHALL bootstrap its stable shim, application claim, and initial verified executable output from a `github_release` source using the managed installation contract in [application-lifecycle](../application-lifecycle/spec.md). Routine self-update SHALL use the managed update contract, committing the selected claim and binding together after validation. It SHALL NOT overwrite a running executable, replace the stable shim, or clone Saucepan's Git repository. The shim SHALL reside outside evictable output directories.

#### Scenario: Saucepan updates itself on Windows
- **WHEN** a running Saucepan instance acquires and activates a new release
- **THEN** the existing executable's bytes remain untouched, future shim launches use the new output, and no repository checkout is created

### Requirement: Recovery and rollback preserve truthful activation state
An interrupted activation SHALL recover a complete old or new binding, never a partially written one. Binding-only rollback through `activate` SHALL leave claims unchanged. Managed `rollback` SHALL additionally move the selected claim, preserve its original request and enforce its item identity, as defined in [application-lifecycle](../application-lifecycle/spec.md). Both forms SHALL require an available validated output; unavailable rollback targets SHALL require reacquisition or report failure. Shim maintenance SHALL follow [shim-maintenance](../shim-maintenance/spec.md) independently of routine managed-release updates.

#### Scenario: A retained release is selected for rollback
- **WHEN** a caller activates an available validated previous release
- **THEN** future shim launches resolve that entry and its active protection applies

### Requirement: The built application completes the simulated installation flow
Acceptance verification SHALL use the actual built Saucepan executable and built shim with an explicit repository-local simulated root. The initial reimplementation recipe SHALL support `mock/` directly; subsequent lifecycle regression runs SHALL preserve that existing installation and use a fresh isolated root, as defined in [lifecycle-recovery](../lifecycle-recovery/spec.md). The built application SHALL be packaged and acquired through the release provider using a controlled release fixture, indexed, claimed, activated with `useable_app`, and launched through the stable shim. Manually authored success records or stand-in executables SHALL NOT substitute for this flow. Test endpoint selection SHALL be explicit and SHALL NOT change the production provider default.

#### Scenario: A local build is acquired and launched as a managed release
- **WHEN** the initial mock recipe runs simulated bootstrap against a loopback release fixture serving the packaged local build
- **THEN** `mock/store/<entry-uuid>/` contains executable bytes matching that build's digest, `mock/index/` records the entry and application binding, and `mock/bin/saucepan` or `mock/bin/saucepan.exe` launches that selected output
- **AND** the resulting simulated installation remains available for inspection

#### Scenario: The simulated installation updates between real builds
- **WHEN** a second distinguishable build is acquired and activated through the same simulated flow
- **THEN** the shim launches the newly selected build, a failed candidate leaves the previous binding usable, and eviction respects active protection
- **AND** verification records actual build identities and outputs rather than inferring a version change from release tags alone
