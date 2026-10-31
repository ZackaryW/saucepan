# Tasks

## 1. Source model and store contract (checkpoint: new types register and query)

- [x] 1.1 Replace `Source::Git` with `GitRepo { origin, token_file }`, add `GithubRaw`, `GitRaw` (with `GitPlatform`) and `GithubSource`, and keep `GithubRepo` as a persisted type; verify serde round-trip unit tests for every variant and that `{"type":"git"}` fails to deserialize
- [x] 1.2 Rewrite `core/source/git.rs` normalization to canonicalize within each type only (GitHub repo lowercase, `git_repo` origin, `git_raw` HTTPS host and multi-segment repo, contained `path`, `token_file` rejected on non-HTTP(S) origins); verify unit tests for each `invalid_request` scenario in raw-file-sources and source-resolution
- [x] 1.3 Add the `repository(source) -> Option<(origin, credential kind)>` helper and switch every `Source::Git { origin }` match (`retrieve`, `retrieval/{caller,access,fresh,managed}`, `store/materialize`, `claim/matching`, `group/plan`, `source/composition`) to it; verify `cargo test --test git --test retrieval --test retrieval_cache` passes using `git_repo` and `github_repo` fixtures
- [x] 1.4 Change `CONTRACT` to `saucepan.platform-sources` and report `saucepan.flat-policy-resources` roots as `unsupported_store` with fresh-root guidance; verify a test opening a preceding-contract root fails without mutating it (store-layout scenario)
- [x] 1.5 Extend `SourceType`, `query/source.rs` matching and `query-sources --type` spellings to the new type set, dropping `git`; verify `cargo test --test query --test source_query_cli` covers the "Removed git type" and "Repository filter spans GitHub types" scenarios
- [x] 1.6 Update the CLI help text and `docs/source-query.md` for the new type spellings; verify the documented example command runs against a test root

## 2. Credentials and authenticated HTTP (checkpoint: github_release uses gh)

- [x] 2.1 Add `ProviderToken` (redacting `Debug`, no `Serialize`/`Display`) and credential resolution: `gh auth token --hostname github.com` with a 10 s timeout for `github_*`, `token_file` with `~` expansion for `git_*`; verify unit tests with a fake `gh` (`test_gh`) for success, missing executable, non-zero exit, timeout and the missing/empty token-file failure
- [x] 2.2 Apply credentials to provider HTTP (Bearer for GitHub, `PRIVATE-TOKEN` for GitLab, `token` for Gitea) and map 401/403, plus anonymous GitHub 404, to `source_error` with `provider`, `http_status` and `token_sent` details; verify loopback-server tests assert headers, error details and that no token text appears in error output
- [x] 2.3 Switch `github_release` asset download to `/repos/{repo}/releases/assets/{id}` with `Accept: application/octet-stream`, keeping recorded provenance identical; verify `cargo test --test remote` covers authenticated, anonymous and redirect cases, including a test that the redirected request carries no `Authorization` header
- [x] 2.4 Verify a denied refresh preserves the prior output and selection; verify with an acquisition-updates test where the due retrieval returns 401

## 3. Authenticated Git clones (checkpoint: github_repo and git_repo authenticate)

- [x] 3.1 Add optional credentials to `Git::command`, set host-scoped `http.<origin>/.extraHeader` Basic auth through `GIT_CONFIG_COUNT/KEY/VALUE` after clearing ambient config (`x-access-token` for `github_repo`, `oauth2` for `git_repo`); verify a test asserts the token is absent from child argv and from the bare repository config
- [x] 3.2 Wire `github_repo` to the `gh` credential and `git_repo` to `token_file` only; verify tests that `git_repo` on a github.com origin never invokes the fake `gh`, and that an authenticated clone against a local smart-HTTP fixture or recorded header succeeds
- [x] 3.3 Manually verify `git_repo` token clones against one GitLab and one Gitea repository and record the result in the change notes; resolve the design's open question if Gitea rejects `oauth2`

## 4. Raw file sources (checkpoint: github_raw and git_raw acquire files)

- [x] 4.1 Implement GitHub commit resolution (`/commits/{ref}` with `application/vnd.github.sha`, skipped for `commit` selectors) cached in source discovery by selector; verify `retrieval_cache` tests for freshness reuse, `always` refresh and pinned commits
- [x] 4.2 Implement `github_raw` retrieval from the raw endpoint at the resolved commit as file material with `Resolved::Git`; verify tests for the "Branch file is retrieved" and "Exact commit is pinned" scenarios through a loopback `test_github_raw`
- [x] 4.3 Implement `git_raw` URL building for GitLab (`/-/raw/{ref}/{path}`) and Gitea (`/raw/{ref}/{path}`) with `Resolved::Url` provenance; verify loopback tests for both platforms and the unsupported-platform rejection
- [x] 4.4 Enforce natural single-file acquisition for raw types and reject folder selection; verify the raw-file-sources acquisition scenarios
- [x] 4.5 Document `github_raw` and `git_raw` in `README.md`, including the GitLab nested-namespace `repo` form; verify examples parse with `saucepan` registration in a test root

## 5. GitHub source archives (checkpoint: github_source extracts folders)

- [x] 5.1 Add `strip_single_root` to `extract_zip`, requiring exactly one top-level directory and mapping folder selection beneath it, and make link-entry errors name the entry; verify `tests/utilities/archive.rs` covers stripping, folder selection, multi-root rejection and link naming
- [x] 5.2 Implement `github_source` retrieval (commit resolution plus `/zipball/{commit}`) as ZIP file material with `Resolved::Git` and no bare repository; verify a loopback test asserts no `source/<uuid>/repo` is created
- [x] 5.3 Restrict `github_source` acquisition to natural format with root stripping and optional folder; verify the source-archives scenarios, including LFS pointer pass-through
- [x] 5.4 Document `github_source` limits (no submodules, LFS pointers only, symlinks rejected) in `README.md`; verify the docs match the test-covered behavior

## 6. SDKs, release notes and integration

- [x] 6.1 Update Python and TypeScript `Source`/`SourceQuery` models and contract fixtures for the new types and removed `git`; verify `python sdk/check_packages.py` and the SDK test suites pass
- [x] 6.2 Update `docs/releases.md` and `docs/managed-setup.md` with the breaking store contract, the fresh-root/reinstall steps and the Powerspec pairing; verify the `unsupported_store` message names the documented reinstall command
- [x] 6.3 Run `cargo test`, `cargo clippy -- -D warnings` and `scripts/acceptance.py` end to end; verify all pass and record any skipped network-dependent checks

## Verification notes

- 3.3 Gitea (1.x self-hosted, HTTP): `git_repo` with `token_file` cloned a 350-commit, 894-file repository and retrieved `ref main` to an exact commit. A wrong token is refused and the real token accepted with the `oauth2` username. No token text appears anywhere in the store; only the `token_file` path is recorded. No credential prompt appeared.
- 3.3 GitLab: skipped at the user's direction; no GitLab host or token was available. The `oauth2` username matches GitLab's documented token form but is unverified here.
- Observed during 3.3, outside this change: launched from Git Bash (MSYS), the same retrieve stalled indefinitely after the first `fsck` with no child process; launched from PowerShell it completed in 191 s. Each repository extraction pass spawns one `git cat-file` per blob (about 90 s for 894 files), and retrieval runs two passes.

- 6.3: `cargo test` (56 suites), `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, Python SDK (21) and TypeScript SDK (14) tests and `scripts/test_setup.py` (13) pass. Release fixtures in `scripts/` and `sdk/contracts/fixtures/` now serve the asset API path. `test_setup.py`'s busy-launcher case now passes `--root` instead of opening the developer's real `~/.saucepan`.
- 6.3 follow-up: `scripts/acceptance.py` was pre-existing broken (inline `filters`/`policies` removed by the flat-policy-resources contract, unchanged at HEAD `324282b`). It was ported to flat policies (`9fd974d`, passing end to end) and then replaced by `python -m acceptance` in change redesign-acceptance-harness, which also covers git_raw and the borrowed gh token through the real binary.
- Live checks: `github_raw` (`cli/cli` README at `trunk`) and `github_source` (`octocat/Hello-World` at `master`) retrieved through real GitHub with the borrowed `gh` token. `git_raw` was verified only through loopback fixtures.

## Workflow follow-up

- Prepare the paired Powerspec change that registers `github_repo`/`git_repo` instead of `git`.
- Archive this change after review, then run `pspec state clear --change git-platform-sources`.
