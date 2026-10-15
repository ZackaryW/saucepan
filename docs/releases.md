# Release builds

[← Saucepan guide](../README.md#managed-applications-and-releases)

The workflows in `.github/workflows/` adapt the previous Saucepan repository's
release and native-platform checks to the Rust manager and shim. The separate
`sdk-checks.yml` verifies thin Python/TypeScript adapters; SDK installation does
not build or download the Rust manager.

## Checks and publication

`core-platforms.yml` runs on manual dispatch or an explicit reusable-workflow
call. The release workflow does not invoke it. Pull requests and ordinary branch pushes do not start hosted checks;
this keeps the full platform matrix an explicit CI expense. It runs formatting, Clippy,
all-target tests and packaging checks on Windows, macOS and Linux. Ordinary tests
use basic stores and need no native credential service. Optional `check_keyring`
checks exercise native custody separately; only those Linux checks provision an
isolated D-Bus/Secret Service session. A separate
Windows job checks all targets with the declared minimum Rust 1.89.
Native test runs stream test output with `--nocapture` and enable Rust
backtraces so early panic details remain in the log even if a later test hangs
and the job is cancelled.

For a manual run, select `platform` (`all`, `windows`, `macos`, or `linux`)
and choose whether to include `check_msrv`. For example, selecting `linux`
and clearing the MSRV checkbox runs only the Linux job. `check_keyring` defaults
to false; select it only when testing native custody on configured runners.
The equivalent GitHub CLI command for basic Linux checks is:

```sh
gh workflow run core-platforms.yml -f ref=v0.10.0 -f platform=linux -F check_msrv=false
```

Use `ref` to check a specific commit or tag; leaving it empty checks the selected
workflow ref. Platform/MSRV/keyring selectors apply to checking workflows only.
A failing check fails that run; it is not an automatic publication gate. A
selective run does not establish that omitted platforms pass on a new revision.

SDK checks are also separate manual or explicitly reusable workflows. Select
`language` (`python`, `ts`, or `both`) and `platform` on `sdk-checks.yml`:

```sh
gh workflow run sdk-checks.yml -f ref=v0.10.0 -f language=python -f platform=windows
```

The selected job builds one native manager, runs only the requested SDK checks
and checks real-binary operation coverage. `both` adds cross-language consumer
fixtures. These jobs do not repeat Rust all-target tests per language or build
six target architectures per SDK. The Node SDK needs Node 22+, and Python checks
use the minimum Python 3.12. Optional `check_packages` defaults to false; enabling
it adds one Linux job that builds and installs both SDK packages, checks source
installs and exercises an explicit native binary. This package job checks both
languages independently of the runtime language/platform selection:

```sh
gh workflow run sdk-checks.yml -f ref=v0.10.0 -f language=both -f platform=linux -F check_packages=true
```

To retry failures on the same revision, GitHub's
[Re-run failed jobs](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs?tool=cli)
option avoids repeating successful jobs (`gh run rerun RUN_ID --failed`).
Reruns use the original commit, so dispatch a new run to verify newly committed fixes.

Local equivalents (the second and third commands require native custody):

```sh
cargo test --locked --all-targets -- --nocapture
cargo test --locked --test native_keys -- --ignored --nocapture
cargo test --locked --test metadata_integrity native_keyring_round_trip_and_cleanup -- --ignored --exact --nocapture
```

Native checks cover concurrent stores, key round-trip/cleanup and the actual
manager/shim sharing a protected root. Each native test has a bounded worker;
backend unavailability fails a selected test. Omitted native checks are reported
as not requested, not verified. Windows native custody has local evidence;
the recent macOS native-keychain failure is not resolved by making it optional.

`release.yml` runs on a pushed `v*` tag or manual dispatch with an **existing** tag.
The tag must equal `v` followed by the version in that tag's `Cargo.toml`, such as
`v0.10.0` or `v0.10.0-rc.1`. Both paths check out the tag, resolve it to a commit,
and build that same commit. Manual dispatch does not package the latest
default-branch source under an unrelated version. Prerelease tags produce
GitHub prereleases, which Saucepan's `latest_stable` selector excludes.
The tagged commit must include this repository's release scripts and workflows.

Release packaging starts after tag validation. It does not run the core test
matrix, MSRV, native keyring, SDK runtime or package-installation checks. Run the
checking workflows separately for the commit you want to verify. The platform
build matrix still produces the native binaries being published: each build
uses `Cargo.lock`, builds both binaries, and sets `SAUCEPAN_BUILD_ID` to the tag.
Publication waits for every target build, the universal package, SDK packaging
and exact asset/manifest/checksum validation. Only the publication job has `contents: write`.
Existing release assets are not overwritten; use a new version/tag for changed
bytes. Release notes are generated by GitHub.

## Assets

Each asset is named `saucepan-<tag>-<target>.zip`. The retained target matrix is:

| Platform | Targets |
| --- | --- |
| Windows | `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`, `i686-pc-windows-msvc` |
| macOS | `x86_64-apple-darwin`, `aarch64-apple-darwin`, `universal-apple-darwin` |
| Linux | `x86_64-unknown-linux-musl` |

Each ZIP has a flat root containing `saucepan`, `saucepan-shim` and `LICENSE`.
Windows executables include `.exe`. Both macOS universal binaries are combined
with `lipo`; the shim is not omitted. ZIP entries record executable permissions
explicitly. Archives are packaged before artifact upload, preserving the internal
mode metadata despite [artifact transfer's permission behavior](https://github.com/actions/upload-artifact#permission-loss).
The platform-independent SDK archives are built once from that same commit:
Python wheel and source distribution, TypeScript npm tarball and
`sdk-manifest.json`. The manifest records both SDK package versions, their exact
filenames and hashes, and the Rust tag/commit. This identifies the source being
packaged; it does not assert that optional checks passed. SDK versions come from
their own package metadata and can differ from Rust's version.

The optional SDK checking job installs archives outside the checkout and exercises
the selected binary, source installation and declarations. The publication validator
requires the exact seven ZIPs plus those three SDK archives and manifest,
cross-checks package/source identities and contents, rejects unsafe or duplicate
members and refuses mixed-commit manifests. Unexpected files/directories fail
validation; it does not blindly publish a `dist/*` glob. No npm/PyPI publication
is configured.

`SHA256SUMS` records hashes for all eleven payloads including the manifest. These are checksums,
not publisher signatures or notarization.

For a Windows x64 GitHub release request, use an asset pattern such as
`saucepan-*-x86_64-pc-windows-msvc.zip` with `selection.platform` set to
`{"os":"windows","arch":"x86_64"}`, `format: "zip"`, and
`bin: ["saucepan.exe"]`. The shim is included for initial bootstrap or explicit
launcher maintenance. Routine managed self-update still changes the selected
application output rather than replacing the installed shim.

## Local checks

The CI-only packaging helper uses Python 3.11+ standard-library archive/TOML support;
Python is not a Rust manager runtime dependency. The separately installed Python
SDK requires Python 3.12+.
Run its focused checks from the repository root:

```powershell
uv run --no-project python -B -m unittest discover -s scripts -p 'test_*release.py'
```

The tests check both binaries, archive executable modes, checksum contents, tag
validation, incomplete build sets, missing shims, SDK metadata/source mismatch,
unsafe/duplicate members, mixed commit evidence and overwrite refusal. Workflow
syntax can be checked with `actionlint`.

Hosted workflow results are separate from local packaging checks. Validate the
target platforms and the release-download/bootstrap/update/rollback flow before
claiming end-to-end acceptance. Dated results belong to the relevant OpenSpec
change; see [specifications and change history](../README.md#specifications-and-change-history).

To publish, commit the intended version and lockfile, push that commit, then push
the matching tag to the destination GitHub repository. This workflow work does
not itself create a remote, push commits or tags, or publish a release.
