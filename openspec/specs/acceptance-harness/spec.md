# acceptance-harness Specification

## Purpose
Run real-build acceptance of the manager, shim and SDK against controlled provider fixtures in isolated roots, with bounded, diagnosable execution and honest reporting of the environments covered.

## Requirements

### Requirement: Scenarios run in isolated roots and home directories
Each acceptance scenario SHALL run against a fresh root created for that scenario, and every manager invocation it makes SHALL name that root explicitly. While scenarios run, the user home seen by the manager and its children (`HOME`, `USERPROFILE`, and the platform's configuration and cache homes) SHALL point at a temporary directory owned by the run. Acceptance SHALL NOT open, create, modify or delete the user's real default root or the repository's historical `mock/` root.

#### Scenario: A launcher started without --root stays inside the sandbox
- **WHEN** a scenario launches a manager or shim without `--root` while the developer has a real `~/.saucepan` using an earlier store contract
- **THEN** the process resolves its default root inside the run's temporary home, not the developer's root
- **AND** the developer's root is byte-for-byte unchanged after the run

#### Scenario: Scenarios do not share state
- **WHEN** two scenarios run in the same invocation
- **THEN** each observes only the sources, policies, claims and outputs it created

### Requirement: One provider fixture serves every provider route
Acceptance SHALL use a single loopback provider fixture for GitHub release discovery, release asset downloads through the asset API, commit resolution, source snapshot archives and raw files, and for GitLab and Gitea raw-file routes. Scenarios SHALL declare releases, files and failures as data, and SHALL be able to read the requests the fixture received, including which carried an authorization header. The fixture SHALL listen on loopback only. Scenarios SHALL NOT start ad hoc provider servers.

#### Scenario: Provider route changes are fixed once
- **WHEN** the manager changes which provider route it uses to download a release asset
- **THEN** updating the shared fixture is sufficient for every scenario that downloads releases

#### Scenario: Injected provider outage
- **WHEN** a scenario marks the provider as failing
- **THEN** subsequent discovery requests receive a server error until the scenario clears the failure

### Requirement: Manager test endpoints cover every exercised provider
The manager SHALL accept hidden, loopback-only overrides for the GitHub API, GitHub raw-file and Git platform raw-file endpoints, and for the `gh` executable used to borrow a token. A non-loopback override SHALL fail with `invalid_request`. When any endpoint override is present and no `gh` override is given, the manager SHALL NOT invoke the real `gh`. These overrides SHALL NOT change production defaults or appear in help output.

#### Scenario: Platform source acquired through the real binary
- **WHEN** a scenario registers a `github_raw` source and acquires it with the GitHub API and raw-file overrides pointing at the fixture
- **THEN** the file is downloaded from the fixture and no request leaves loopback

#### Scenario: Fixture gh supplies the token
- **WHEN** a scenario passes a `gh` override that prints a fixture token
- **THEN** the fixture observes that token on GitHub requests and the developer's own `gh` login is never consulted

#### Scenario: Non-loopback override rejected
- **WHEN** an endpoint override names a non-loopback host
- **THEN** the command fails with `invalid_request` before any network request

### Requirement: Scenarios drive the manager through the language SDK
Behavioral scenarios SHALL call the manager through the in-repository Python SDK for every operation the SDK provides. Operations the SDK deliberately does not expose, such as launcher activation and shim maintenance, SHALL go through one shared, typed adapter owned by the harness. Scenarios SHALL NOT compose manager argv or request JSON themselves, so a contract change absorbed by the SDK or the adapter needs no scenario edits.

#### Scenario: Contract drift surfaces once
- **WHEN** a manager request or result shape changes and the SDK is updated for it
- **THEN** behavioral scenarios run unchanged

#### Scenario: Operation outside the SDK
- **WHEN** a scenario activates a launcher, an operation the SDK does not expose
- **THEN** it calls the harness adapter, which is the only place that spells that command's arguments

### Requirement: CLI smoke checks keep literal argv coverage
A separate CLI smoke group SHALL exercise argument parsing, explicit root precedence, the JSON error envelope and exit codes through literal argv, so command-line regressions hidden by the SDK are still detected.

#### Scenario: Error envelope through argv
- **WHEN** the smoke group runs a command with an invalid request using `--error-format json`
- **THEN** it observes a non-zero exit and a JSON error object with a code and message on stderr

### Requirement: Time is controlled explicitly
Scenarios SHALL control the manager's clock through the explicit test clock and SHALL exercise refresh, expiry and stale behavior by advancing it past policy windows together with registered policies, not through per-call override flags or wall-clock waits.

#### Scenario: Discovery expires
- **WHEN** a scenario advances the clock past the 24-hour default freshness window
- **THEN** the next update performs a new discovery request against the fixture

### Requirement: Stalled invocations fail with diagnostics
Every manager, shim and consumer invocation made by acceptance SHALL have a deadline. When a deadline expires, the run SHALL terminate the invocation's process tree and fail that scenario with the command, its elapsed time, the processes that were still alive with their parents, and any captured output, then continue with the remaining scenarios.

#### Scenario: A manager process blocks after its child exits
- **WHEN** a manager invocation stops making progress after its last child process exited
- **THEN** the scenario fails at its deadline with a report naming the manager command and showing no live child processes
- **AND** later scenarios still run

### Requirement: Scenarios are independent and selectable
Acceptance SHALL be organized as independent scenarios with tags at least for release lifecycle, freshness, retention, platform sources, setup and CLI smoke. A run SHALL be able to select scenarios by tag or name. Binaries that need distinct build identities SHALL be built once per run and shared by the scenarios that use them. One scenario's failure SHALL NOT prevent other selected scenarios from running and reporting.

#### Scenario: Run one tag
- **WHEN** a developer selects only the platform-sources tag
- **THEN** only those scenarios run and the report lists the others as not selected

### Requirement: The integrated lifecycle remains one coherent scenario
The release-lifecycle scenario SHALL keep the integrated real-build lifecycle required by self-management and lifecycle-recovery as one coherent sequence. Splitting other behaviors into separate scenarios SHALL NOT remove any step of that sequence. Its two distinguishable builds SHALL be two locally stamped builds when building from source, and the candidate plus its verification twin in artifacts mode.

#### Scenario: Lifecycle scenario completes
- **WHEN** the release-lifecycle scenario runs
- **THEN** it bootstraps a packaged build through the fixture, updates to distinguishable bytes, preserves a failed candidate, rolls back, protects a busy launcher from reclaim and records the resulting identities

#### Scenario: Lifecycle in artifacts mode
- **WHEN** the scenario runs against a candidate and its verification twin
- **THEN** it updates from one to the other without building, and the report records both digests

### Requirement: Reports state what actually ran
Each run SHALL write a machine-readable report naming the platform, the launch environment (for example PowerShell, Git Bash or POSIX shell), the build identities and digests, and each scenario's outcome as passed, failed, skipped or not selected, with failure diagnostics. Reports SHALL NOT claim coverage of environments, platforms or native keyring custody that did not run.

#### Scenario: Windows run under Git Bash
- **WHEN** acceptance runs on Windows from Git Bash
- **THEN** the report records Windows and Git Bash, and does not claim PowerShell or Linux results

### Requirement: Hosted checks run acceptance in each launch environment
Release verification SHALL run acceptance in artifacts mode on Windows from PowerShell, on Windows from Git Bash, on macOS and on Linux, each producing its own report. The manual platform checks SHALL run acceptance from source in the same environments, keep their manual or explicitly invoked triggers, and let every platform finish rather than cancelling on the first failure.

#### Scenario: Launch-environment-specific failure
- **WHEN** a defect only reproduces when the manager is launched from Git Bash
- **THEN** the Git Bash acceptance job fails while the PowerShell job may pass, and both reports identify their environment

### Requirement: Artifacts mode tests packaged releases without building
Acceptance SHALL accept a directory of downloaded release assets and, in that mode, SHALL use the host platform's packaged manager and shim, the candidate's own installers and `SHA256SUMS`, and SHALL NOT invoke a compiler. The report SHALL record the candidate tag and the digest of every archive it used. Building from source SHALL remain the default when no artifacts are given.

#### Scenario: No compiler in artifacts mode
- **WHEN** acceptance runs with a candidate's assets on a machine without a Rust toolchain
- **THEN** every selected scenario runs against the packaged binaries and none attempts a build

#### Scenario: Setup uses the candidate installers
- **WHEN** the setup scenarios run in artifacts mode
- **THEN** the installer scripts and checksums they execute are byte-identical to the candidate's release assets

### Requirement: Release upgrade follows the store contract
A release upgrade scenario SHALL install the previous published release through setup and self-update it to the candidate through the provider fixture, determining both store contracts from the roots each release creates. With the same contract, the upgraded installation SHALL run store commands successfully; with different contracts, the scenario SHALL assert the documented `unsupported_store` break instead.

#### Scenario: Same-contract upgrade
- **WHEN** the previous release and the candidate share a store contract
- **THEN** self-update succeeds and the upgraded launcher queries the existing store

#### Scenario: Contract-changing upgrade
- **WHEN** the candidate introduces a new store contract
- **THEN** store commands after self-update fail with `unsupported_store`, the message names the reinstall procedure, and the previous root's files are unchanged
