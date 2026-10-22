# Implementation verification and consumer handoff

Verified locally on Windows 11, 2026-10-21. The implementation is uncommitted on
baseline `6a4cd063e74df72c7cc02aeb848bce36558036c8`; that commit does **not** contain
these changes. No release was published and the production Saucepan home was not
migrated. Fixture tags such as `v0.11.0` are synthetic, not release evidence.

## Delivered contract

- Setup/status schema version 1, documented in [managed setup](../../../../docs/managed-setup.md).
- Administrative `setup` with explicit release tag, durable operation identity,
  verified current-state reuse, linked lifecycle recovery and retained migration backups.
- Read-only `setup-status`; explicit `self-update --tag` commits the replacement
  request, output and binding together.
- Matching Python and TypeScript setup/status types, exit-2 decoding and optional
  self-update tag, documented in [SDK usage](../../../../sdk/managed-setup.md).
- Release-rendered PowerShell and POSIX installers, included in the exact asset
  allowlist and checksums. Scripts require a setup-capable provider release.

## Executed checks

| Check | Result and evidence |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `cargo test --locked --all-targets` | Passed; `target/setup-cargo-tests.log`; native custody tests remain ignored |
| Focused `cargo test --lib core::setup` | 5 passed, including intent tampering and migration crash states |
| Actual CLI/installer acceptance | 12 passed; `target/setup-cli-tests.log` |
| Rendered installer and Git sh checks | 3 passed, overlapping two of the preceding cases; `target/setup-installer-tests.log` |
| Added duplicate ZIP rejection | Fresh installer case rerun passed after adding this assertion |
| Release packaging checks | 13 passed |
| Python SDK suite and mypy | 21 tests passed; 17 source files type checked |
| TypeScript SDK suite/build/types | 14 tests passed |
| Expanded actual-provider SDK fixtures | 5 tests per adapter passed; `target/setup-python-resource-tests.log`, `target/setup-ts-resource-tests.log` |
| SDK contract inventory | 59 operations covered per adapter |
| Installed SDK distributions | Wheel, sdist and npm package checks passed outside the checkout, including imports with empty toolchain PATH; `target/setup-package-tests.log` |
| OpenSpec strict validation and whitespace review | `openspec validate add-managed-setup --strict --no-interactive` and `git diff --check` passed |

The actual manager/shim tests cover fresh setup, no-network reuse, exact tag
updates and failed-update preservation, migration and backup identity, busy
Windows executables, concurrent setup, interrupted acquisition and durable-shim
retry, altered retry inputs, damaged launchers, and unrelated-file preservation.
Core tests additionally exercise incomplete backup and post-displacement states.
SDK fixtures invoke the real provider for setup, reuse, migration and tag update.
Installer tests exercise literal Unicode/space paths, malformed archives,
checksum/capability failure, and noncomplete receipt forwarding.

`target/setup-acceptance.json` accumulates run observations, including native
artifact hashes, operation/application/claim/output identities and migration
backup digests. Temporary test stores are removed after assertions; reported
paths are historical evidence, not live installations. Retained backups means
they survived the operation inside each fixture before fixture cleanup.

## Pre-version-bump local artifact identity

`target/setup-handoff.json` records the baseline, uncommitted state, native and
SDK package hashes, SDK source digests, and installer template digests. These
identify local builds only; they do not supply an immutable production revision.
This table records the original verification build before the authorized version
bump. Native paths have since been rebuilt; current hashes are recorded below.

| Local artifact | SHA256 |
| --- | --- |
| `target/debug/saucepan.exe` | `888f5b9059707675e15887016deb6f2146893f5e13a350696e748f53c18bab83` |
| `target/debug/saucepan-shim.exe` | `a1246282ac8b73449c1b2e437fac3e3113d78a16547369a0545821dc92469ad4` |
| `target/setup-sdk-dist/saucepan-sdk-0.2.0.tgz` | `16856a5ba40c1d59407f9aeeedfd03f7aef715fad7c6e249a8eb64efdfa22233` |
| `target/setup-sdk-dist/saucepan_sdk-0.2.0-py3-none-any.whl` | `388373d64f0e4d36fb3a181b4fc1a792c6efed08fcea7e437f8be34a1682abca` |
| `target/setup-sdk-dist/saucepan_sdk-0.2.0.tar.gz` | `a1fe5fd8be2ef8d840b757185072f76d55fa94e125834c8ea476d6cae61ceae8` |

## Authorized version bump

Manager/shim manifests and Cargo.lock now identify 0.11.0; Python and TypeScript
SDK manifests and their lockfiles identify 0.3.0. Dependency versions were not
changed. README, SDK installation examples and release workflow examples use
the new versions while explicitly identifying them as unreleased. Historical
v0.10.0 incompatibility and baseline evidence remain intact.

`cargo build --locked --bins` passed and the rebuilt manager reports
`saucepan 0.11.0+dev`. The Python lock check and all 13 release packaging tests
passed. The rendered installer fresh-setup/preflight case passed against these
binaries (`target/version-bump-installer.log`). New SDK packages were built into
`target/version-0.11.0-sdk-dist`; build output is in
`target/version-bump-package-build.log`.
Installed wheel, sdist and npm package verification passed against the rebuilt
manager/shim, including lifecycle execution outside the checkout and TypeScript
consumer types (`target/version-bump-package-verify.log`). OpenSpec strict
validation and `git diff --check` also passed after the bump. The earlier full
behavior suites were not repeated for these version/documentation-only changes;
the platform and key-custody limits below still apply.

`target/version-bump-handoff.json` records the current local build identities:

| Local artifact | SHA256 |
| --- | --- |
| `target/debug/saucepan.exe` | `f4d57f9cf0c29cb593d3505944e18d7239f909e451708ad1436db42b58b5382b` |
| `target/debug/saucepan-shim.exe` | `16c2e65d4fd5940ad98f482c77d4847aa3cef2046a5dd0e317c9a72a32465126` |
| `target/version-0.11.0-sdk-dist/saucepan-sdk-0.3.0.tgz` | `e74635951a1cd996c2a651ccd3f2aa74b418333f53dde928245374dd04c5f2c6` |
| `target/version-0.11.0-sdk-dist/saucepan_sdk-0.3.0-py3-none-any.whl` | `b17e2c246ac642dd61bd512631db87fd3d81beaf4a4121f593543702ad786336` |
| `target/version-0.11.0-sdk-dist/saucepan_sdk-0.3.0.tar.gz` | `042ac94ec62ad9890bfd957dfe9e3bbfbb48904101cf16933ec73e7a92007825` |

Proposed version checkpoint: `chore(release): prepare manager 0.11.0 and SDKs 0.3.0`.
The selected zmem executable still exposes the older Python interface, so
computed-memory validation remains unavailable. No commit, tag or publication
was made as part of this version bump.

## Coverage limits and next consumer step

Native Linux/macOS execution, native key custody, and the minimum Rust toolchain
were not run. The POSIX test uses Git sh, a Linux platform-dispatch stub, and
actual Windows binaries; it verifies the shell contract but is not native Unix
acceptance. The selected/manual platform workflow now includes setup acceptance.
No available Linux Docker engine was running, and no service was started.

The linked Powerspec `adopt-saucepan-managed-setup` change can develop against
these explicit local fixtures and schema 1. Its production release pin and
released-artifact acceptance must remain pending until a real setup-capable
release and compatible immutable SDK revision exist. Published v0.10.0 does not
provide this contract. Installing an SDK alone cannot add native commands.

Proposed commit: `feat(setup): add managed installation and verified raw-manager migration`.
No commit was created. zmem message validation was unavailable because the
installed Python CLI does not expose the required Node-engine validation
contract; Git/source review is not claimed as computed-memory validation.

## Archive closure

On 2026-10-21, all 21 tracked tasks and all planning artifacts were complete.
Ten added requirements and two modified requirements were synced into seven
main capabilities; every delta requirement was checked against the merged
content, preserving existing scenarios. Strict validation passed for all 27 main
specifications and for the change before archival. The change was then archived
as `2026-10-21-add-managed-setup`, and `pspec state clear --change add-managed-setup`
succeeded with no temporary-state changes needed. Earlier uncommitted/no-commit
statements above describe those verification checkpoints; this archive is being
included in the subsequently authorized implementation commit. Release and
native-platform verification limits remain unchanged.
