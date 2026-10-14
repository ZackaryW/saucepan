# sdk-distribution Specification

## Purpose

Distribute the Python and TypeScript SDKs from this repository with independently installable artifacts and verified parity against the Rust executable from the same release commit.

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

### Requirement: Release packages are tied to verified source and binary evidence
SDK archives SHALL be built from the exact release commit and accompany the existing binary assets. A validated SDK manifest SHALL record package versions, artifact identities and the tested Rust release tag/commit. Release validation SHALL check the complete expected binary and SDK asset set and include all published payloads in checksum coverage. Missing, unexpected, duplicate or inconsistent artifacts SHALL block publication. This workflow SHALL not publish to npm or PyPI or infer arbitrary executable compatibility from a version string.

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
Hosted SDK checks SHALL run only on manual selection or as release dependencies, with selectable language/platform for manual runs. Release SDK validation SHALL cover supported Windows/macOS/Linux process boundaries against the release commit without multiplying the full Rust test matrix per SDK. Platform-independent archives SHALL be built once per release. Planning and implementation SHALL not automatically trigger hosted runs or package publication.

#### Scenario: Selected manual check
- **WHEN** the user requests only Python SDK checks on Windows
- **THEN** unrelated TypeScript and other-platform SDK checks are not scheduled by that selection

#### Scenario: SDK verification fails before release
- **WHEN** a required SDK contract or installed-package check fails
- **THEN** the complete release publication job does not proceed
