# Proposal

## Why

Releases are published the moment a tag builds, with no test of the artifacts users download, while the platform checks compile everything again on every platform (the last hosted run spent over nine minutes per platform in `cargo test` before any acceptance ran). Compiling once, testing the exact packaged binaries, and withdrawing a candidate that fails would both cut verification time and ensure nothing unverified reaches Powerspec, the official installers or Saucepan's own self-update.

## What Changes

- The release workflow stages a **draft** release from the binaries it builds once, then verifies those exact draft assets on Windows (PowerShell and Git Bash), macOS and Linux without compiling.
- When every platform passes, the draft is **promoted** by publishing it unchanged: same assets, digests, installers and `SHA256SUMS`.
- When any stage before promotion fails or is cancelled, the candidate is **revoked**: the draft is deleted, the version tag is moved to `revoked/<tag>-<short-commit>`, and the reports are kept. Verification jobs fail fast, so one platform failure cancels the rest.
- A published release is never revoked; a defect after publication is fixed by a new version.
- The build stage also produces one internal "verification twin" per verified platform (same commit, distinct build identity) so the lifecycle scenario can update between two builds of the candidate. Twins are never attached to the release.
- The acceptance harness gains an **artifacts mode** that takes downloaded release assets instead of building, including the candidate's own installers and checksums, and a **release upgrade** scenario that self-updates from the previous published release to the candidate. When the store contract changed between them, it asserts the documented `unsupported_store` break and reinstall guidance instead of a working upgrade.
- A new push and pull-request workflow runs formatting, Clippy, `cargo test` and the harness self-tests on Linux only. **This reverses** the current rule that ordinary pushes start no checks.
- `core-platforms.yml` remains the manual, all-platform, from-source diagnostic workflow, with fail-fast off so every platform reports.

## Capabilities

### New Capabilities
- `release-verification`: how a release candidate is staged, verified on each platform from its packaged artifacts, promoted unchanged or revoked, and how failures are reported and reproduced.

### Modified Capabilities
- `sdk-distribution`: publication now depends on artifact verification; ordinary pushes and pull requests run the Linux check workflow; checking workflows stay manually selectable.
- `acceptance-harness`: adds artifacts mode, the release upgrade scenario, verification twins for the lifecycle scenario, and release verification as a hosted context.

## Impact

- **Workflows**: `.github/workflows/release.yml` (draft stage, verify matrix, promote, revoke), new `.github/workflows/ci.yml`, `.github/workflows/core-platforms.yml` (fail-fast off; unchanged triggers).
- **Harness**: `acceptance/` builds (artifact source), sandbox and scenarios, `scripts/test_setup.py` (candidate binaries and installers in artifacts mode).
- **Packaging**: `scripts/release.py` twin packaging, kept out of the release allowlist.
- **Consumers**: none. Powerspec pins exact tags and installer digests, and Saucepan setup and self-update resolve releases through `releases/tags`, `releases/latest` and `releases/download`; drafts are invisible to all of them and promotion preserves bytes.
- **Not covered**: Windows ARM64 and macOS x86_64 artifacts are packaged but have no native verification runner; reports say so. Native `cargo test` on Windows and macOS runs only in the manual workflow.
