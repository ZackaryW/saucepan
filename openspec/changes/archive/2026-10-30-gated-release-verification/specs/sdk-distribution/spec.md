# Spec Delta

## MODIFIED Requirements

### Requirement: Release validation includes installer identities and selective acceptance
Release packaging SHALL include both installer scripts in its exact artifact allowlist and checksum manifest, verify their tag/source consistency, and reject missing, extra or mixed-release artifacts. Existing published assets SHALL not be overwritten. Publication SHALL follow acceptance of the packaged artifacts as defined in release-verification. Manual checking workflows SHALL separately exercise installer and SDK/setup contracts from source using real built binaries, isolated roots and controlled release fixtures on the selected platforms. Packaging SHALL not imply that runtime checks passed.

#### Scenario: Installer script missing from a release
- **WHEN** a setup-capable release lacks one required script or its embedded tag differs
- **THEN** publication validation fails before publishing an incomplete set

#### Scenario: Selected platform acceptance
- **WHEN** only Windows installer/setup verification runs
- **THEN** results identify Windows evidence and do not claim Unix or native-keyring acceptance

### Requirement: SDK validation preserves selective CI execution
Native Rust, minimum-version, optional keyring, SDK runtime and SDK installation checks SHALL remain separately selectable manual checking workflows that accept a target commit or tag and their applicable platform/language selectors. SDK archive installation and source-install verification SHALL be an explicit option, disabled by default, that checks both packages on one platform. Release workflows SHALL build platform-independent SDK archives once and SHALL NOT depend on the manual checking workflows. Ordinary pushes and pull requests SHALL run only the Linux check workflow (formatting, Clippy, Rust tests and harness self-tests) and SHALL NOT publish packages. An omitted check SHALL not be represented as passed.

#### Scenario: Selected manual check
- **WHEN** the user requests only Python SDK checks on Windows
- **THEN** unrelated TypeScript and other-platform SDK checks are not scheduled by that selection

#### Scenario: Release without optional verification
- **WHEN** a release is triggered for an existing valid version tag
- **THEN** it builds and verifies the artifact set without scheduling native Rust tests, keyring checks, SDK contract tests or SDK installation tests

#### Scenario: Explicit package installation check
- **WHEN** a caller enables package checks for a selected commit or tag
- **THEN** the checking workflow verifies installed archives and source installs for both SDKs on one platform, separately from release publication

#### Scenario: Verification failure is reported independently
- **WHEN** a selected SDK or native check fails
- **THEN** that checking workflow reports failure without becoming an automatic release dependency or implying that unselected checks passed

#### Scenario: Push runs only the Linux checks
- **WHEN** a commit is pushed to a branch or a pull request is updated
- **THEN** only the Linux check workflow runs, and no Windows or macOS job, release or package publication is scheduled
