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
Acceptance verification SHALL use the actual built Saucepan executable and built shim with an explicit isolated simulated root. The initial reimplementation used `mock/` directly; current regression runs SHALL preserve that historical installation and use a fresh basic-mode root under the current explicit integrity contract, as defined in [lifecycle-recovery](../lifecycle-recovery/spec.md). The built application SHALL be packaged and acquired through the release provider using a controlled release fixture, indexed, claimed, activated with `useable_app`, and launched through the stable shim. Manually authored success records or stand-in executables SHALL NOT substitute for this flow. Test endpoint selection SHALL be explicit and SHALL NOT change the production provider default. Historical release-flow results SHALL NOT by themselves establish that this combined flow has been verified against a later store contract. Basic acceptance SHALL complete without a keyring service. A separately selected protected manager/shim integration SHALL record native custody results; successful basic acceptance SHALL NOT be represented as verification of native keyring support.

#### Scenario: A local build is acquired and launched as a managed release
- **WHEN** a fresh-root recipe runs simulated bootstrap against a loopback release fixture serving the packaged local build
- **THEN** that root's `store/<entry-uuid>/` contains executable bytes matching the build's digest, its `index/` and `claims/` record the output, application binding and claim, and its stable shim launches that selected output
- **AND** verification records the resulting identities and preserves the historical `mock/` installation

#### Scenario: The simulated installation updates between real builds
- **WHEN** a second distinguishable build is acquired and activated through the same simulated flow
- **THEN** the shim launches the newly selected build, a failed candidate leaves the previous binding usable, and eviction respects active protection
- **AND** verification records actual build identities and outputs rather than inferring a version change from release tags alone

### Requirement: First setup is explicit and supports profile-prepared release requests
Saucepan SHALL document and support first setup beginning with an explicitly obtained release manager and matching built shim. The flow SHALL explicitly initialize the selected root, register ordinary profile-expanded installation policies and GitHub release source/filter records, and invoke the existing bootstrap operation. Profile choice SHALL not clone a repository, choose a different identity/scope contract, alter verification modes or replace normal platform selection. The default root SHALL retain existing user-home .saucepan resolution. Setup instructions SHALL describe adding its bin directory to PATH without editing shell profiles or environment settings automatically. Installing a language SDK SHALL NOT perform these steps.

#### Scenario: Basic setup without a keyring
- **WHEN** a caller explicitly initializes a basic root and bootstraps from a GitHub release request with registered explicit-update installation refs
- **THEN** the existing release acquisition, version probe, managed claim and stable shim flow succeeds without a keyring requirement

#### Scenario: SDK package installed without a manager
- **WHEN** a user installs or imports a language SDK with no manager present
- **THEN** no Saucepan root, binary, source, claim or launcher is created and attempted manager operations report the missing executable

### Requirement: Profile acceptance exercises the real self-management boundary
Profile-assisted self-management acceptance SHALL use the actual built manager and shim, an isolated basic root and a controlled GitHub release fixture. It SHALL verify expanded definition identities, launch through the shim, an explicit update to distinguishable executable bytes, failed-candidate preservation and reclaim protection. Historical mock installations and real user roots SHALL remain untouched. Platform and native-keyring evidence SHALL be reported only for checks actually run.

#### Scenario: Profile-assisted release update fails validation
- **WHEN** an isolated installation acquired through for-install policies encounters an invalid new release
- **THEN** the existing selected claim and binding remain usable through the actual shim and the failed operation is inspectable
