## 1. Index entry schema

- [x] 1.1 Add a flattened extra-field map to `BucketStub` in `src/bucket.rs`, keeping `name`, `version`, and `url` required
- [x] 1.2 Add a test that an entry omitting any required field is rejected as invalid
- [x] 1.3 Add a test that an index containing extra fields round-trips without losing them
- [x] 1.4 Add a test asserting the serialized shape stays parseable by the pre-change struct definition

## 2. Central index as a repository target

- [x] 2.1 Extend index registration to accept a repository target in addition to a local path and `file://` URL
- [x] 2.2 Fetch remote indexes through the existing Git and gh path, reusing clone, pull, and authentication
- [x] 2.3 Support an explicit ref on a registered index and record its resolved commit
- [x] 2.4 Replace the `http`/`https` rejection in `fetch_bucket` with repository-target handling, leaving local reads unchanged
- [x] 2.5 Add tests covering registration by repository target, by local path, and by `file://` URL

> Verified by architect: 106 tests passing (60 unit + 46 integration, up from 93), `Cargo.toml`/`Cargo.lock` diff is empty — no new dependency, per the design's central constraint.
> **2.3 closed in group 5**: `bucket add --ref <reference>` now fetches synchronously and persists `resolved_commit` to `buckets.json`, rolling back the registration (`registry_remove`) if that fetch fails — verified by the architect in source (`commands/bucket.rs::add`) — so a failed pin never leaves a half-registered entry. A new `bucket refresh <url>` command re-fetches and updates `resolved_commit` on demand for any registered index. Plain `bucket add` (no `--ref`) and all read commands (`search`, `cat bucket`, the resolution chain) remain network-free / non-mutating, as originally intended.
> Judgment call accepted: a repository-target index is matched to a fetched target via `stub.url == repo_url`. Note this promotes `url` from purely informational (as saucepan's README currently describes it for `search`) to a matching key — a real semantic shift on an existing field, flagged for awareness, not blocking.
> Judgment call accepted: a ref that exists but fails checkout is treated the same as a missing ref (reject entry, continue chain) — an extrapolation beyond the literal spec scenario, consistent with the design's "indexes are auxiliary, non-blocking" stance.
> Bug caught by the implementing agent's own testing, not by review: the first local-vs-repository-target heuristic used `Path::exists()`, true for directories too, misclassifying a local repository-target index as a plain file. Fixed to `is_file()`. Would have broken in production.

## 3. Manifest resolution chain

- [x] 3.1 Introduce an ordered chain abstraction in `src/sources/git.rs` replacing the single manifest existence check
- [x] 3.2 Implement the repository link, preserving current behavior exactly
- [x] 3.3 Implement the central-index link, matching a fetched target against registered index entries
- [x] 3.4 Enforce that a manifest-supplying entry also supplies a ref, rejecting the entry and continuing the chain when it does not
- [x] 3.5 Check out the entry-supplied ref during fetch so the recorded version matches the working tree
- [x] 3.6 Implement deterministic index precedence by registration order, reporting shadowed entries
- [x] 3.7 Warn and skip an index that cannot be fetched or read, without failing the operation
- [x] 3.8 Return the existing not-found result when the chain is exhausted

> Verified by architect: `ManifestLink` trait + `default_chain() -> Vec<Box<dyn ManifestLink>>` = `[RepositoryManifestLink, CentralIndexLink]`, walked by `resolve_manifest`; `Ok(None)` means "try next link" so the chain is genuinely appendable, matching the design's extensibility requirement. `fetch_sauce` calls `clone_or_update` + `resolve_manifest` and `install.rs`/`update.rs` needed zero changes — the chain activates end-to-end through the real CLI.
> Judgment call accepted: index-fetch authentication reuses the *primary* target's `GitFetchOptions` (binary/token/ssl_key), overriding only `reference` per entry; standalone `search`/`cat bucket` (no primary-fetch context) default to unauthenticated `git`. Adequate for install/update; standalone bucket commands need config wiring in group 5.

## 4. Provenance

- [x] 4.1 Add a manifest-source field to the index entry in `src/index.rs`
- [x] 4.2 Populate it from the winning chain link during install and update — currently wired to `repository` at every call site; the chain agent makes `ManifestSource::index()` live
- [x] 4.3 Expose it through `cat index`, `cat sauce`, and `list --json`
- [x] 4.4 Confirm entries written before this change still deserialize with the field absent

> Verified by architect: 93 tests passing (53 unit + 40 integration) against an 88-test baseline. `manifest_source` omitted from the `Local` variant by design — local entries never pass through `fetch_sauce`, so no chain link supplies their manifest, and the SDK's absent→`repository` default reads correctly for them.
> Verification gap: `cargo clippy` and `cargo fmt` are not installed in this environment (bare Scoop cargo, no rustup components), and `.github/workflows/release.yml` only builds release binaries — there is no CI test, clippy, or fmt job. Lint is therefore unverified everywhere, not merely deferred to CI. Correctness rests on `cargo test` alone.

## 5. Command wiring

- [x] 5.1 Wire chain resolution into `src/commands/install.rs`
- [x] 5.2 Wire chain resolution and ref handling into `src/commands/update.rs`
- [x] 5.3 Extend `src/commands/bucket.rs` to register, list, and remove repository-target indexes
- [x] 5.4 Extend `src/config.rs` for any index-related configuration the chain requires

> Verified by architect: 115 tests passing (63 unit + 52 integration, up from 106), `Cargo.toml`/`Cargo.lock` diff still empty.
> **Real bug found and fixed during 5.1/5.2 verification, not by review**: chain resolution itself was already wired correctly, but `GitFetchResult.manifest_source` was being computed and then silently discarded — both `install.rs` and `update.rs` still hardcoded `ManifestSource::repository()` at all four `IndexEntry` construction sites, so every index-resolved install would have recorded false provenance claiming its manifest came from the repository. This directly contradicted the "done" mark on task 4.2. Confirmed fixed in source: all four sites now use `fetched.manifest_source`, and the implementing agent verified end-to-end against the built binary that `cat sauce` on an index-resolved install correctly reports `{"kind":"index","index":"<url>"}`.
> 5.4: standalone bucket commands (`bucket add`/`refresh`, `search`, `cat bucket`) previously had no auth config and were hardcoded to unauthenticated `git`. A new optional `[index]` config section (mirroring `[github]`/`[customgit]`) now backs them; the chain's in-flight `CentralIndexLink` continues reusing the primary source's `GitFetchOptions` (group 3's accepted judgment call, left untouched since a primary fetch is genuinely in flight there). Confirmed by the agent varying `[index] binary = "gh"` and observing it actually change `bucket refresh` behavior.

## 6. Compatibility verification

- [x] 6.1 Add a test asserting a manifest-bearing install produces an unchanged entry, identity, and resolved commit
- [x] 6.2 Add a test asserting behavior is identical to the prior version when no index is registered
- [x] 6.3 Add an integration test installing a manifest-less repository via an index-supplied manifest and pinned ref
- [x] 6.4 Add an integration test that an unreachable index warns and does not fail an otherwise satisfiable install
- [x] 6.5 Verify the documented exit-code to exception mapping is unchanged

> Verified by architect: 118 tests passing (63 unit + 55 integration, up from 115). Only `tests/integration.rs` and `tests/utils/index.rs` were touched this pass — no production code changed, confirming this was pure verification, not a fix.
> 6.1's prior coverage was inadequate (checked version/commit but not `manifest_source` or full identity) and has been strengthened with a competing-index scenario. 6.2 confirms the pre-chain `NotFound` message text is byte-identical. 6.3's prior test could not distinguish "resolved to the pinned ref" from "resolved to HEAD" (single-commit fixture); the new test advances the repo after pinning to prove the distinction. 6.4 and 6.5 were already adequately covered.
> **Binary-vs-Python-SDK cross-check performed and passed**: built the real binary, ran an index-resolved install, and diffed its actual `cat sauce`/`cat bucket` JSON against the fixtures group 7's SDK tests assumed. Match, no drift — closes the gap that group 7 explicitly flagged as unverified.
> Two non-blocking findings recorded, out of this change's scope: (1) a pre-existing Windows `MAX_PATH` issue in `utils::naming::repo_dir` when saucepan runs from a deeply nested workspace root — unrelated to this change, worth its own investigation if it recurs; (2) `CentralIndexLink` calls `index::load_registry` even on installs the repository link resolves, a harmless single-file-existence-check overhead, not a behavior change.

## 7. Python SDK

- [x] 7.1 Expose the manifest-source field on `Sauce` snapshots
- [x] 7.2 Expose extra fields on parsed bucket stubs, defaulting to empty
- [x] 7.3 Confirm existing call signatures, return types, and cache invalidation are unchanged
- [x] 7.4 Rerun the standard-library-only import walk over every SDK module and record the result

> Verified by architect: 35 tests passing (29 pre-existing unmodified + 6 new), `dependencies = []` intact in `sdk/python/pyproject.toml`.
> `BucketStub` subclasses `dict`, so `Bucket.stubs()` keeps dict equality and JSON-serializability while gaining `.name`/`.version`/`.url`/`.extra` — the existing test asserting `bucket.stubs() == stubs` against plain dicts passes unmodified, which is the compatibility evidence for 7.3.
> New tests are fixture-based against the fixed wire contract, since the Rust emitting side was being built concurrently. Group 6 must include an end-to-end check that the binary's real output matches those fixtures.

## 8. Documentation and release

- [x] 8.1 Document the manifest resolution chain and its precedence in the README
- [x] 8.2 Document registering a central index by repository target, including pinning
- [x] 8.3 Document the index entry schema, the required-field compatibility guarantee, and the ref requirement
- [x] 8.4 Note the exit-code behavior change for callers using exit 1 as a manifest-absence detector
- [x] 8.5 Bump the version, sync the lockfile, and release binaries so the downstream zush migration can pin them

> Verified by architect: 118 tests passing (unchanged from group 6), `Cargo.toml` now `0.3.0`, only `Cargo.toml`/`Cargo.lock`/`README.md`/`sdk/python/README.md` touched this pass — no production source regressed.
> **8.5 is only half-actionable and remains a human step**: the version *number* is settled at `0.3.0` in the working tree, but no commit, tag, or binary release exists yet — this task cannot be fully closed by an agent, since committing/tagging/publishing was explicitly out of scope for every lane in this change. A human must commit, tag `v0.3.0`, and cut the release before anything can fetch it.
> **Downstream handoff to zush-2** (currently paused): `src/zush/vendor/saucepan_binary.py` line 28's `PINNED_SAUCEPAN_VERSION = "v0.2.0"` placeholder should become `"v0.3.0"` once that tag exists and is fetchable — v0.2.0 predates this entire change and cannot satisfy zush-2's dependency on it. Do not update the zush-2 constant before the tag is real; pinning a version that doesn't exist yet is worse than leaving the placeholder.

## 9. Lean-audit follow-up (post-completion)

- [x] 9.1 Shrink `ManifestLink` from a private trait + `Box<dyn ManifestLink>` dynamic dispatch (two closed, in-crate implementors, never `pub`) to a private enum matched in a loop

> Prompted by `/governance-of-agents-1v2:lean-audit` against this completed change, converged with the owner. Grounded check against zush-2's proposal/design/specs confirmed zush interacts with saucepan only as an external binary + Python SDK — nothing registers a Rust type into this chain, and the one deferred future link (the generator hook) is explicitly framed in design.md as an in-crate code addition, not external/dynamic registration. No spec anywhere justified open dispatch, so the trait-object indirection was cut.
> Verified by architect: 118 tests passing, unchanged from pre-refactor (63 unit + 55 integration). Only `src/sources/git.rs` touched. Every warning string, condition, and rejection branch confirmed byte-identical; both rationale doc comments (repository-always-wins, index precedence/shadowing/warn-skip/ref-rejection) relocated onto the enum variants rather than dropped.
> A separate finding from functional validation — target-matching by exact string equality on `url` — was subsequently fixed; see group 10.

## 10. Target-matching normalization (post-completion correctness fix)

- [x] 10.1 Normalize repository-target strings before comparing them in the central-index link, in `src/utils/naming.rs` alongside the existing target-string helpers

> **The bug**: `ManifestLink::CentralIndex` matched a registered index's stub to the install target with exact string equality (`stub.url != repo_url`). Equivalent spellings of the *same* repository — a `.git` suffix, a trailing separator, `\` vs `/`, or any of the three GitHub forms (`owner/repo`, `https://github.com/owner/repo`, `git@github.com:owner/repo`) — silently failed to match, producing no warning and looking identical to "no index describes this target." Found by the architect during functional validation against the real binary, not by the test suite.
> **Fix**: `naming::normalize_target` canonicalizes for comparison only; stored and displayed strings are untouched (verified against the binary — `repo` still records the original spelling). Placed in `src/utils/naming.rs` at the owner's direction, matching the existing `repo_dir`/`terminal_component` convention.
> **Deliberate restriction**: case is folded ONLY for recognized GitHub-shaped targets, since github.com treats `owner/repo` case-insensitively. Local paths and custom-git URLs keep their case — folding those could produce a *false* match between genuinely different repositories on a case-sensitive filesystem or Git server, which is strictly worse than the missed match being fixed. Two negative tests enforce this (`normalize_target_does_not_fold_case_for_a_non_github_local_path`, `..._for_a_custom_git_ssh_host`).
> **Process note — caught by evaluation, not by green tests**: the implementing agent was interrupted after writing `normalize_target` and its 7 unit tests but *before* wiring the call site in `git.rs`. The suite went 118 → 125 passing and stayed green, because the new unit tests exercised `normalize_target` directly and passed whether or not anything called it. The bug remained fully live behind a passing build. The architect wired the call site and verified end-to-end against the binary.
> Verified by architect: 126 tests passing (70 unit + 56 integration). `git.rs:208` confirmed holding the normalized comparison after the regression test's temporary revert-and-restore. Regression test `install_matches_index_stub_url_differing_from_target_by_a_trailing_slash` was proven meaningful by observing it FAIL against the old exact-equality comparison (`no sauce.json found in ...`) before passing with the fix restored.
