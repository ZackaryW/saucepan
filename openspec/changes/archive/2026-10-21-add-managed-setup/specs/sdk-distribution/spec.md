# Spec Delta

## ADDED Requirements

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
