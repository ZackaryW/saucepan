# Tasks

## 1. Harness artifacts mode (checkpoint: acceptance runs against packaged archives without cargo)

- [x] 1.1 Add `--artifacts`, `--twin` and `--previous` to `python -m acceptance`, and an artifact source in `acceptance/builds.py` that unpacks the host archive (same target map as setup) for `manager()` and returns candidate and twin for the lifecycle labels; verify a harness self-test that artifacts mode never invokes cargo (stand-in cargo that fails) and selects the host archive
- [x] 1.2 Record `candidate` (tag, per-archive SHA-256), `previous` tag and packaged-but-unverified targets in the report; verify with a self-test reading the written report
- [x] 1.3 Make the setup suite take candidate binaries, `install.ps1`, `install.sh` and `SHA256SUMS` from the artifacts directory and serve them through the forge under the candidate tag; verify locally with archives produced by `release.py package` that `--tag setup` passes in artifacts mode and the served installers are byte-identical to the inputs
- [x] 1.4 Run the lifecycle scenario on candidate plus twin in artifacts mode; verify locally with two archives packaged from builds stamped with different `SAUCEPAN_BUILD_ID`s

## 2. Release upgrade scenario (checkpoint: contract-aware upgrade evidence)

- [x] 2.1 Add the `release_upgrade` scenario (tag `release`): serve previous and candidate releases as `ZackaryW/saucepan`, install the previous one through setup, compare the store contracts from roots each release initializes, self-update, then assert a working upgrade or the documented `unsupported_store` break with reinstall guidance and an unchanged previous root; skip with a reported reason when no previous archive is given; verify locally against the published v0.10.0 archive (contract change expected) and against a same-contract pair (two local packages)

## 3. Twin packaging (checkpoint: twins exist and never reach the release)

- [x] 3.1 Add twin packaging to `scripts/release.py` (`verify-twin-<target>.zip`) that is excluded from `installers`/`checksums` allowlists and rejected if present in the release directory; verify with `scripts/test_release.py` cases for packaging and rejection

## 4. Workflows (checkpoint: gated pipeline and Linux CI defined)

- [x] 4.1 Add `.github/workflows/ci.yml` (push and pull_request, ubuntu-latest: fmt, Clippy, `cargo test --locked --all-targets`, harness self-tests) and set `fail-fast: false` explicitly in `core-platforms.yml`; verify both parse and `ci.yml` has no Windows or macOS job
- [x] 4.2 Add twin entries to the `build` matrix (`x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, `x86_64-unknown-linux-musl`, `SAUCEPAN_BUILD_ID=<tag>-verify`) uploading `verify-twin-<target>` artifacts; verify the release allowlist step excludes them
- [x] 4.3 Replace `release` with `stage` (same validation, `draft: true`), add the `verify` matrix (`fail-fast: true`; windows pwsh, windows bash, macos, linux; no Rust toolchain; download draft assets, twin and previous release; check `SHA256SUMS`; run artifacts-mode acceptance; upload reports; write a per-job summary), `promote` (publish the draft unchanged with matching prerelease/latest flags) and `revoke` (`if: failure() || cancelled()`, refuse published releases, delete draft, push `revoked/<tag>-<short-commit>`, delete the tag, write the pathway summary with reproduction commands); verify the workflow parses and the job graph matches the design's pathway table
- [x] 4.4 Document the gated pipeline, revoke pathways, `revoked/` tags and local reproduction in `docs/releases.md` and `docs/acceptance.md`; verify the documented commands match the workflow

## 5. Rehearsal (checkpoint: revoke and promote observed on GitHub)

- [x] 5.1 With the user's go-ahead, rehearse on a prerelease branch (`0.11.0-rc.1`): one tag with a forced verification failure (observe cancel, draft deletion, `revoked/` tag, summary) and one passing tag (observe promotion with unchanged asset digests); record run links and delete the rehearsal prerelease

## Workflow follow-up

- Fix the macOS lease-marker failure separately using the diagnostics added in `be09419`.
- Update Powerspec's pinned v0.11.0 installer digests after the verified release is promoted.
- Archive this change after review.

## Verification notes

- 1.x–2.1 local (Windows x64): artifacts mode with a locally packaged candidate and twin passed setup (14) and lifecycle (8.3 s, no build); release_upgrade passed against the published v0.11.0 archive (contract change: v0.11.0 completes self-update, then the candidate refuses the flat-policy-resources root) and against a same-contract pair.
- 5.1 rehearsal on `rehearsal/v0.12.0-rc.1` (version `0.12.0-rc.1`):
  - Run 41179837610 (forced failure): revoke cancelled the other verify jobs, deleted the draft and moved the tag to `revoked/v0.12.0-rc.1-557708c89ba5`. It also exposed that read-only verify jobs cannot read drafts; fixed in `34b542f` (stage proves the draft equals the staged set; verify tests the staged artifact).
  - Run 41180371086 (forced failure after the fix): Linux acceptance ran on the packaged artifacts (26 passed incl. upgrade from v0.11.0 and the twin lifecycle; only the forced scenario failed); revoke summary named it with a working reproduction command.
  - Run 41180783413 (passing): all four verify jobs passed (Linux 26, macOS 26, Windows PowerShell 28, Windows Git Bash 28; skips are Windows-only tests); promote published a prerelease, v0.11.0 stayed Latest, and the 14 published assets were byte-identical to the staged set.
  - Cleanup: rehearsal prerelease, `v0.12.0-rc.1`, both `revoked/` tags and the branch deleted.
- After the rehearsal, per-job reports listed every other packaged target as unverified; promote and revoke summaries now list only targets no verify job exercised (Windows ARM64/x86, macOS x86_64 and universal).
