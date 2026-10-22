# sdk-distribution Specification

## Purpose

Distribute independently installable Python and TypeScript SDK artifacts from the Rust release commit, with separately selectable workflows for runtime and installation verification.

## Requirements

### Requirement: Python supports local and Git-subdirectory source installation
The package under sdk/python SHALL support local source installation, editable development and commit-pinned Git-subdirectory installation through Python packaging tools. Wheel and source-distribution artifacts SHALL contain the importable package, typing metadata, README and license, and SHALL work outside the repository checkout. Installation/import SHALL not require Cargo, build/download Saucepan or initialize a store. Native binary selection SHALL occur only at explicit operation time.

#### Scenario: Install from a source checkout
- **WHEN** a user installs sdk/python locally or through a commit-pinned subdirectory URL
- **THEN** the package imports from the resulting environment and can use an explicitly selected executable

#### Scenario: Install a standalone Python artifact
- **WHEN** a clean environment without Rust tooling installs the wheel or source distribution
- **THEN** installation and import succeed without accessing files elsewhere in the checkout or downloading the manager

### Requirement: TypeScript supports built local source and complete npm artifacts
The package under sdk/ts SHALL provide a documented source build and installable npm tarball containing runtime JavaScript, declarations, README and license. A consumer of the tarball SHALL not need a TypeScript compiler or Rust tooling. Package exports SHALL resolve from a clean consumer outside the checkout, including the transport-independent client. Documentation SHALL distinguish local-source installation and tarballs from unsupported direct npm Git-subdirectory installation. No native-binary installation hook SHALL run.

#### Scenario: Consume a packed SDK
- **WHEN** a separate project installs the generated tarball and imports the client
- **THEN** JavaScript and declarations resolve without reaching into sdk/ts/src or running native installation steps

### Requirement: Release packages are tied to source and artifact identity
SDK archives SHALL be built from the exact release commit and accompany the existing binary assets. A validated SDK manifest SHALL record package versions, artifact identities and the Rust release tag/commit. The manifest SHALL identify artifact provenance without claiming that optional runtime checks ran. Release validation SHALL check the complete expected binary and SDK asset set and include all published payloads in checksum coverage. Missing, unexpected, duplicate or inconsistent artifacts SHALL block publication. This workflow SHALL not publish to npm or PyPI or infer arbitrary executable compatibility from a version string.

#### Scenario: Incomplete SDK artifact set
- **WHEN** a release is missing a required SDK archive or its manifest disagrees with package metadata
- **THEN** release validation fails before publishing a partial asset set

#### Scenario: Mixed release commit
- **WHEN** SDK artifact evidence identifies a different source commit from the release being prepared
- **THEN** validation rejects the mixed artifact set

### Requirement: Both SDKs are verified through actual CLI contracts
Both languages SHALL have a checked operation inventory and real-binary tests using explicit executables and isolated stores. Tests SHALL exercise success, typed domain failures, context boundaries, read-only lookup, composition, groups/pins and guarded execution. At least one fixture SHALL create resources in one language and inspect/use/release them through the other under matching authority. Packaging smoke tests SHALL run from outside the source trees. Test reports SHALL distinguish isolated consumer-shaped evidence from actual Power Eagle, Zush or PowerSpec integration and identify native-keyring/platform exclusions.

#### Scenario: Cross-language retained generation
- **WHEN** Python prepares and pins a group and TypeScript inspects its receipt and exact output paths
- **THEN** both observe the same generation and explicit release obeys the same owner and retention rules

#### Scenario: Read-only PowerSpec-shaped lookup
- **WHEN** a fixture queries an existing project selection while upstream content has changed
- **THEN** lookup does not refresh it, and explicit preparation can advance only the intended consumer scope

### Requirement: SDK validation preserves selective CI execution
Native Rust, minimum-version, optional keyring, SDK runtime and SDK installation checks SHALL be separately selectable checking workflows, independent of release publication. Checking workflows SHALL accept a target commit or tag and support their applicable platform/language selectors. SDK archive installation and source-install verification SHALL be an explicit option, disabled by default, that checks both packages on one platform. Release workflows SHALL build platform-independent SDK archives once and validate artifact integrity without invoking or depending on these checking workflows. An omitted check SHALL not be represented as passed. Ordinary branch pushes SHALL not automatically start checks or publish packages.

#### Scenario: Selected manual check
- **WHEN** the user requests only Python SDK checks on Windows
- **THEN** unrelated TypeScript and other-platform SDK checks are not scheduled by that selection

#### Scenario: Release without optional verification
- **WHEN** a release is triggered for an existing valid version tag
- **THEN** it builds and validates the complete artifact set without scheduling native Rust tests, keyring checks, SDK contract tests or SDK installation tests

#### Scenario: Explicit package installation check
- **WHEN** a caller enables package checks for a selected commit or tag
- **THEN** the checking workflow verifies installed archives and source installs for both SDKs on one platform, separately from release publication

#### Scenario: Verification failure is reported independently
- **WHEN** a selected SDK or native check fails
- **THEN** that checking workflow reports failure without becoming an automatic release dependency or implying that unselected checks passed

### Requirement: Official installer launchers hand off verified release binaries
Saucepan SHALL publish PowerShell and POSIX shell installer scripts from the same tagged commit as the manager/shim. Each script SHALL identify its release and accept explicit release/root/operation selectors and explicit migration inputs. It SHALL resolve only supported published platform assets, validate the exact archive checksum from that release, and stage the matching manager and shim outside the selected store before execution. Invalid checksums, missing/duplicate/escaping archive entries, unsupported platforms, unavailable prerequisites and a manager lacking setup SHALL fail without publishing a raw binary or silently using a different version.

Scripts SHALL use literal argument passing, preserve setup JSON/exit results and report diagnostics separately. They SHALL not implement store policy/receipt/ownership logic, install SDKs, require a Rust/Python/Node toolchain, alter PATH/shell profiles, elevate privileges, bypass execution policy, or automatically retry mutation. Cleanup SHALL be limited to script-owned scratch; durable inputs, receipts and backups SHALL remain available for recovery. An older release without setup SHALL not be installed through a hidden fallback.

#### Scenario: Download and bootstrap an exact release
- **WHEN** a user runs a supported installer for an exact setup-capable tag
- **THEN** the matching verified manager/shim execute setup with that tag and only successful managed publication is reported as installation

#### Scenario: Archive validation fails
- **WHEN** a release ZIP has the wrong digest or invalid required members
- **THEN** no contained executable is run and the selected store is unchanged

#### Scenario: Version lacks setup
- **WHEN** an installer targets v0.10.0 or another binary without the setup contract
- **THEN** it reports the unsupported version instead of copying the manager into the launcher path

#### Scenario: Paths contain spaces and Unicode
- **WHEN** root, shim, or staging paths contain spaces or Unicode
- **THEN** the intended literal paths reach setup and cleanup does not escape its owned scratch directory

### Requirement: Release validation includes installer identities and selective acceptance
Release packaging SHALL include both installer scripts in its exact artifact allowlist and checksum manifest, verify their tag/source consistency, and reject missing, extra or mixed-release artifacts. Existing published assets SHALL not be overwritten. Checking workflows SHALL separately exercise installer and SDK/setup contracts using real built binaries, isolated roots and controlled release fixtures on the selected platforms. Packaging SHALL not imply those optional runtime checks passed, and ordinary pushes SHALL not automatically schedule all checks or publish.

#### Scenario: Installer script missing from a release
- **WHEN** a setup-capable release lacks one required script or its embedded tag differs
- **THEN** publication validation fails before publishing an incomplete set

#### Scenario: Selected platform acceptance
- **WHEN** only Windows installer/setup verification runs
- **THEN** results identify Windows evidence and do not claim Unix or native-keyring acceptance
