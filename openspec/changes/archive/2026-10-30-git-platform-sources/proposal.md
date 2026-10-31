# Proposal

## Why

Saucepan reaches GitHub anonymously: release discovery hits the 60 requests/hour limit and private releases or repositories are unreachable, while the only way to obtain a few repository files is a full `git clone`. Consumers such as Powerspec need authenticated GitHub access through the login the user already has (`gh`), cheaper ways to fetch a single file or a source snapshot, and a way to reach GitLab and Gitea hosts.

## What Changes

- **BREAKING** Replace the `git` source type with `git_repo { origin, token_file? }`. It clones from any host, including GitHub, with plain Git and an optional manually assigned token file. No alias, decoder or migration for `git` is kept.
- **BREAKING** `github_repo { repo }` becomes a persisted type of its own instead of normalizing to `git`. Its clones authenticate with the token borrowed from `gh`.
- `github_release` borrows the `gh` token for discovery and downloads assets through the GitHub asset API by asset id, so private releases work. Recorded provenance is unchanged.
- New `github_raw { repo, path }`: one file from a GitHub repository at a ref, tag or exact commit, resolved to a commit through the GitHub API.
- New `git_raw { host, platform, repo, path, token_file? }`: one file from a GitLab or Gitea host through that platform's raw-file URL.
- New `github_source { repo }`: a source snapshot ZIP of a GitHub repository at a resolved commit, downloaded through the GitHub API without history; acquisition extracts it with optional folder selection.
- All `github_*` types obtain a token from `gh auth token` when `gh` is installed and logged in, otherwise they proceed anonymously. All `git_*` types read their token from `token_file` when configured. HTTP 401/403 from a provider fails with a `source_error` whose details identify the provider status and how to authenticate.
- Tokens are held in memory only and never written to records, journals, results, logs or error text.
- **BREAKING** Source records change shape, so this release introduces a new named store contract. Stores using the preceding contract fail with `unsupported_store` and require a fresh root.
- GitLab and Gitea release and source-archive types are out of scope for this change.

## Capabilities

### New Capabilities
- `provider-credentials`: how provider requests obtain, apply, protect and report credentials, covering the borrowed `gh` token for `github_*` types and `token_file` for `git_*` types.
- `raw-file-sources`: the `github_raw` and `git_raw` single-file source types, their selectors, provenance and platform URL rules.
- `source-archives`: the `github_source` snapshot ZIP type, commit resolution, root-folder stripping and its documented limits.

### Modified Capabilities
- `source-resolution`: GitHub release assets download through the asset API with provider credentials; repository retrieval is defined for `git_repo` and `github_repo` instead of `git` and a normalized `github_repo`.
- `source-query`: stored type spellings and provider field matching cover the new type set and drop `git`.
- `store-layout`: a new named store contract identifies stores with the new source records; the preceding flat-policy contract becomes unsupported.

## Impact

- **Rust core**: `models/source` (`Source`, `Resolved` usage), `core/source/{git,remote,retrieve,composition}.rs`, `core/retrieval/{managed,fresh,caller,access}.rs`, `core/store/materialize.rs`, `core/claim/matching.rs`, `core/group/plan.rs`, `core/query/source.rs`, `utils/source/git.rs` (authenticated clone/fetch), `utils/archive.rs` (single-root stripping), `core/claim/persistence.rs` (store contract).
- **CLI**: `query-sources --type` spellings; source registration inputs.
- **SDKs**: Python and TypeScript `Source` and `SourceQuery` models in `sdk/python/src/saucepan_sdk/models/source.py` and `sdk/ts/src/models/source.ts`, plus contract fixtures.
- **External tools**: optional runtime use of the `gh` CLI.
- **Dependencies**: adds the `base64` crate (0.22, within the project's Rust 1.89 floor) for Git Basic authentication headers.
- **Consumers**: Powerspec registers `{"type": "git"}` and must move to `github_repo` or `git_repo` in a paired release. Existing roots, including a managed `~/.saucepan`, need a fresh root after upgrade.
