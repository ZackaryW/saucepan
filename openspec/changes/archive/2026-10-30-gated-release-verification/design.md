# Design

## Context

See proposal.md (Why). Current state:

- `release.yml` on a `v*` tag: `prepare` (tag and commit metadata) → `sdk-packages` and `build` (matrix of six targets) → `macos-universal` → `release`, which validates the set (`release.py installers`, `checksums`) and publishes immediately with `softprops/action-gh-release`, marking tags that contain `-` as prereleases. Nothing is tested.
- `core-platforms.yml` (manual) compiles and runs fmt, Clippy, `cargo test` and acceptance from source on Windows, macOS and Linux with `fail-fast: false`.
- No workflow runs on push or pull request.
- Self-management and Powerspec resolve releases only through `releases/download/<tag>/…`, `releases/tags/<tag>` and `releases/latest`. Powerspec pins the exact tag and the installer digests. Setup picks `saucepan-<tag>-<target>.zip` for the host (`aarch64-apple-darwin` on macOS arm64, `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-musl`).
- The acceptance harness builds the debug manager and two labeled release builds itself.

## Goals / Non-Goals

**Goals:**
- Compile once per target per release; test the exact draft assets.
- No unverified release visible to any consumer; promotion that preserves bytes.
- One deterministic revoke path for every pre-promotion failure, with reports and a local reproduction command.

**Non-Goals:**
- Fixing the macOS lease-marker test failure (separate work; it does not run in release verification).
- Verifying Windows ARM64, Windows i686, macOS x86_64 or the universal macOS archive natively.
- Changing asset names, installer contents, checksum format or the release allowlist.
- Migrating earlier store contracts (decided: assert the documented break).

## Decisions

### Pipeline shape

```
tag v* --> prepare --> sdk-packages ----------------------+
                  \--> build (6 targets + 3 twins) --+     |
                                     macos-universal +--> stage (draft)
                                                           |
                verify: windows-pwsh, windows-gitbash, macos, linux  (fail-fast)
                                                           |
                               all success --> promote (publish draft)
            any failure/cancel before promote --> revoke
```

- `stage` replaces `release`. It runs the same validation, then creates the release with `draft: true` and the same allowlisted files. Twins are uploaded only as workflow artifacts (`verify-twin-<target>`) and never to the draft.
- `verify` is one matrix job (`fail-fast: true`) with entries `{windows-latest, pwsh}`, `{windows-latest, bash}`, `{macos-latest, bash}` and `{ubuntu-latest, bash}`. Each entry checks out the tagged commit (for the harness and scripts only), sets up Python, downloads the `staged-release` artifact (stage has already confirmed with `verify_release.py check-draft` that the draft holds exactly those bytes; drafts are only readable with write access, which verify jobs do not get), downloads its twin artifact and the previous published release's host archive, verifies the archive against `SHA256SUMS`, then runs `python -m acceptance --artifacts dist --twin twin --previous previous`. No Rust toolchain is installed.
- `promote` (`needs: verify`, runs only on success) runs `gh release edit <tag> --draft=false` with `--prerelease` matching the tag and `--latest` for stable tags. Assets are untouched.
- `revoke` (`needs: [prepare, build, macos-universal, stage, verify]`, `if: failure() || cancelled()`) refuses if `gh release view <tag> --json isDraft` reports a published release. Otherwise it deletes the draft if present (`gh release delete <tag> --yes`, keeping the tag), pushes `refs/tags/revoked/<tag>-<short-commit>` at the candidate commit, deletes `refs/tags/<tag>`, and writes the run summary. It needs `contents: write`. Because it runs after promotion is skipped, it can never race a publication.

Alternatives: promote by re-running `softprops` with the final settings. Rejected because it can re-upload and change digests. Give verify jobs write access so they can download the draft directly. Rejected because they run test code; the stage job proves draft and staged bytes are identical instead (found in the first rehearsal, where a read-only verify job could not see the draft).

### Verification twins
The lifecycle scenario needs two distinguishable builds of the same commit and store contract. The build matrix gains three entries (`x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, `x86_64-unknown-linux-musl`) that build with `SAUCEPAN_BUILD_ID=<tag>-verify` and package as `verify-twin-<target>.zip`. They run in parallel with the other targets, so wall time barely changes.

Alternatives:
- The previous release as the second build. Rejected because it fails whenever the store contract changes, as it does in 0.11.0.
- Sibling targets (i686 on x64 Windows, universal on arm64 macOS). Rejected because Linux has none.
- Patching the build identity into the binary. Rejected because it invalidates macOS signatures.

### Harness artifacts mode
`python -m acceptance --artifacts DIR [--twin DIR] [--previous DIR]`:
- `Builds` gains an artifact source. `manager()` unpacks the host archive from `DIR` (selected with the same target map as setup). `release(label)` returns the candidate for the first label and the twin for the second. The cargo paths are untouched when `--artifacts` is absent.
- The setup suite takes the candidate binaries plus `install.ps1`, `install.sh` and `SHA256SUMS` from `DIR` and serves them through the forge under the candidate tag. The fabricated malformed-tag cases keep using candidate binaries.
- New `release_upgrade` scenario (tag `release`): the forge serves the previous release and the candidate as `ZackaryW/saucepan` releases. The scenario installs the previous one with its own setup, reads the `contract` from both releases' roots (each initializes a scratch root), then self-updates. The assertion follows the contract comparison, per the spec.
- The report gains `candidate` (tag and per-archive SHA-256) and `previous` (tag), and lists packaged targets that were not verified.

### Linux check workflow
New `ci.yml` on `push` (all branches) and `pull_request`: one `ubuntu-latest` job running `cargo fmt --check`, `cargo clippy --locked --all-targets -D warnings`, `cargo test --locked --all-targets` and the harness self-tests. `core-platforms.yml` keeps its manual triggers and from-source acceptance, with `fail-fast: false` stated explicitly.

### Revoke pathways

| Failure point | Automatic action | Evidence | Next step |
| --- | --- | --- | --- |
| `prepare` (tag does not match `Cargo.toml`, bad tag) | revoke: move the tag; no draft exists | run log | fix the version or tag, push the tag again |
| `build` or `sdk-packages` (any target) | revoke: move the tag; no draft exists | failing target's log | fix and re-tag |
| `macos-universal` (lipo or packaging) | revoke | job log | fix packaging and re-tag |
| `stage` (allowlist, installer mismatch, checksums, upload) | revoke: delete the partial draft, move the tag | job log | fix `release.py` or assets and re-tag |
| `verify` Windows PowerShell | cancel siblings, revoke | `acceptance-windows-pwsh` report and summary | reproduce on Windows: `python -m acceptance --artifacts <dir> --twin <dir> --previous <dir> --name <scenario>` from PowerShell |
| `verify` Windows Git Bash | cancel siblings, revoke | `acceptance-windows-bash` report and summary (records which `git` was resolved) | same command from Git Bash |
| `verify` macOS | cancel siblings, revoke | `acceptance-macos` report and summary | same command on macOS arm64 |
| `verify` Linux | cancel siblings, revoke | `acceptance-linux` report and summary | same command on Linux x64 |
| Run cancelled by a person during build or verify | revoke | partial reports | re-tag when ready |
| `promote` fails | none: the draft remains and the tag remains | job log | rerun the `promote` job |
| Defect found after publication | none: revoke refuses published releases | user reports | release a new version |

The summary written by `revoke` names the failed job, the platform and launch environment, the failing scenario ids parsed from the uploaded reports, the `revoked/` tag, and the exact `gh release download` / `gh run download` and `python -m acceptance` commands to reproduce. Each verify job also writes its own summary before failing, so cancelled siblings still show what completed.

## Risks / Trade-offs

- [A moved tag is visible to anyone using git tags directly] → Revoke only happens to drafts, minutes after tagging. The `revoked/` tag keeps the commit reachable. `docs/releases.md` documents it. Powerspec pins commits and release assets, not git tags.
- [Fail-fast hides whether other platforms would also fail] → Accepted for the release gate, since any failure revokes anyway. Use the manual `core-platforms.yml` (fail-fast off) for a full picture.
- [Native Windows and macOS `cargo test` no longer runs automatically; the macOS lease-marker failure would not block a release] → Tracked as separate work. The manual workflow still runs it.
- [Twins add three release builds per tag] → They run in parallel matrix slots. They are never published.
- [The previous release may lack an asset for a target] → `release_upgrade` is skipped with a reported reason when no previous published release provides the host archive.

## Migration Plan

1. Land the harness artifacts mode and the `release_upgrade` scenario. Verify them locally against archives produced by `release.py package`.
2. Land `ci.yml` and the `core-platforms.yml` fail-fast change.
3. Replace the `release` job with `stage`, `verify`, `promote` and `revoke`, and add the twin matrix entries.
4. Rehearse on a branch whose `Cargo.toml` version is a disposable prerelease (for example `0.11.0-rc.1`, since `release.py` requires the tag to equal the manifest version). Push `v0.11.0-rc.1` once with a forced verification failure to observe revocation, then again on a fixed commit to observe promotion. Delete the rehearsal prerelease afterwards.

Rollback: restore the previous `release` job. Drafts left by an aborted run can be deleted manually.
