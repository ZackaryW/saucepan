## Purpose

Define the vendorable Python utility that resolves and downloads the correct `saucepan` release binary for the host platform, so other repositories can consume Saucepan's prebuilt binaries without hand-rolling GitHub Releases handling.

## Requirements

### Requirement: Explicit-version binary resolution
The Python platform resolver SHALL require an explicit version string from the caller and SHALL NOT resolve or infer a "latest" version.

#### Scenario: Caller supplies a version
- **WHEN** the resolver is called with a version such as `"v0.2.0"`
- **THEN** it downloads the release asset tagged exactly `v0.2.0` for the detected platform

#### Scenario: Caller omits the version
- **WHEN** the resolver is invoked without a version
- **THEN** it raises a clear error rather than resolving any implicit "latest" release

### Requirement: Platform detection and asset mapping
The resolver SHALL detect the host OS and architecture using the Python standard library and SHALL map the detected platform to the corresponding `saucepan` release asset name.

#### Scenario: Known platform resolves to its asset name
- **WHEN** the resolver runs on a platform present in its asset-name mapping (macOS x86_64/aarch64, Windows x86_64/aarch64/x86, or Linux x86_64)
- **THEN** it selects the exact matching release asset name for that platform

#### Scenario: Unknown platform is rejected clearly
- **WHEN** the resolver runs on an OS/architecture pair absent from its asset-name mapping
- **THEN** it raises an error naming the unrecognized OS/architecture pair instead of guessing an asset name

### Requirement: Direct asset download without the GitHub REST API
The resolver SHALL download release binaries via direct GitHub release-asset URLs and SHALL NOT call the GitHub REST API, and therefore SHALL require no authentication token.

#### Scenario: Resolving a version makes no API call
- **WHEN** the resolver downloads a binary for a given version and detected platform
- **THEN** it requests only the direct release-asset download URL and never calls `api.github.com`

### Requirement: Executable downloaded binary
The resolver SHALL ensure the downloaded binary is executable on POSIX platforms after download.

#### Scenario: POSIX download is made executable
- **WHEN** the resolver downloads a binary on macOS or Linux
- **THEN** the resulting file has the executable bit set

#### Scenario: Windows download needs no permission change
- **WHEN** the resolver downloads a binary on Windows
- **THEN** it does not attempt to change file permissions

### Requirement: Standard-library-only, vendorable module
The resolver SHALL be implemented using only the Python standard library and SHALL be usable both as an importable function and as a directly runnable script, so it can be vendored into another repository (e.g. via git submodule) without a package-manager install step.

#### Scenario: Module has no third-party dependencies
- **WHEN** the resolver module is imported
- **THEN** it requires no packages beyond the Python standard library

#### Scenario: Module is runnable standalone
- **WHEN** the resolver module is executed directly (e.g. `python saucepan_resolver.py v0.2.0`)
- **THEN** it downloads the resolved binary without requiring the caller to import it as a module first

### Requirement: Excluded from the published Rust crate
The `resolvers/python/` directory SHALL be excluded from the published `saucepan` Rust crate package.

#### Scenario: Crate package omits the resolver
- **WHEN** the `saucepan` crate is packaged for publishing
- **THEN** `resolvers/python/` is not included in the package file list
