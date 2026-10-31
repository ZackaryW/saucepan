# Design

## Context

- Today `Source` (`src/models/source/mod.rs`) has `local`, `url`, `github_release`, `github_repo` and `git`. `github_repo` is rewritten into `git` during normalization (`src/core/source/git.rs`), so every repository path matches on `Source::Git { origin }`.
- All provider HTTP goes through one `ureq` agent in `src/core/source/remote.rs`, unauthenticated. Release assets download from `browser_download_url` (`src/core/source/retrieve.rs`).
- Git runs through `utils::source::git::Git::command`, which clears ambient `GIT_CONFIG*` variables and passes hardening settings with `-c`.
- `utils::archive::extract_zip` already supports folder selection, bounds and link rejection.
- Stores are identified by a named contract (`CONTRACT` in `src/core/claim/persistence.rs`); earlier contracts fail with `unsupported_store`.

Requirements live in the change specs: `provider-credentials`, `raw-file-sources`, `source-archives`, and the deltas to `source-resolution`, `source-query` and `store-layout`.

## Goals / Non-Goals

**Goals:**
- Use one credential path per provider family: `gh` for `github_*`, `token_file` for `git_*`.
- Keep retrieval, acquisition, policies and claims provider-agnostic: new types only add a retrieval branch and a material kind already understood downstream.
- Keep recorded provenance independent of whether a token was used.

**Non-Goals:**
- GitLab/Gitea release or archive types.
- Commit resolution for `git_raw`.
- A credential policy type or per-app credential selection.
- Blob-SHA verification of raw files.
- Powerspec changes, which ship as a separate paired change.

## Decisions

### 1. Source model: one variant per type, no normalization between types
```
GitRepo      { origin, token_file: Option<String> }
GithubRepo   { repo }
GithubRelease{ repo }
GithubRaw    { repo, path }
GitRaw       { host, platform: GitPlatform /* gitlab | gitea */, repo, path, token_file: Option<String> }
GithubSource { repo }
```
- `GithubRepo` stays a persisted type, and `normalize` only canonicalizes within a type (GitHub repo lowercased, `git_repo` origin normalized as today). Rewriting it into `git_repo` would lose the "use `gh`" intent, which is why this replaces the current normalization.
- Repository code paths that match `Source::Git { origin }` (`retrieve`, `caller`, `access`, `materialize`, `claim/matching`, `group/plan`, `composition`, `query/source`) switch to a helper: `fn repository(source) -> Option<(origin, Credential)>`, returned for `GitRepo` and `GithubRepo`.
- `token_file` is stored exactly as written. `~` is expanded only when the file is read, so records stay portable across moves of the home directory.
- `GitRaw.repo` allows two or more segments, because GitLab namespaces nest (`group/sub/repo`). GitHub types keep the existing `owner/repo` validator.

*Alternative*: a single `git_platform { platform, kind }` type. Rejected because each kind has different fields and selectors, and serde-tagged variants keep validation declarative.

### 2. Credential resolution
A `Credential` value is resolved once per provider operation and passed into retrieval:
- **`github_*`**: run `gh auth token --hostname github.com` through `utils::process::run` with a 10-second timeout and a 4 KiB capture limit. Ambient `GH_TOKEN`/`GITHUB_TOKEN` are left in place, since `gh` already prefers them. A non-zero exit, missing executable, timeout or empty output yields `Credential::None`.
- **`git_*`**: read `token_file` after `~` expansion and trim it. Missing, unreadable or empty fails with `source_error` before any request (spec: provider-credentials).
- The token is wrapped in a `ProviderToken` newtype with a redacting `Debug`, no `Serialize` and no `Display`, so it cannot reach records, results or `anyhow` contexts by accident.

*Alternative*: shelling to `gh api` for every request. Rejected in exploration because asset selection, streaming and digest checks would move out of Saucepan, and two HTTP paths would need maintaining.

### 3. HTTP authentication and redirects
- **GitHub API and raw requests**: `Authorization: Bearer <token>`.
- **GitLab raw**: `PRIVATE-TOKEN: <token>`.
- **Gitea raw**: `Authorization: token <token>`.
- **Redirects**: the agent keeps `ureq`'s default `RedirectAuthHeaders::Never`, so authorization never follows a redirect, including asset and zipball redirects to download hosts. A test pins this setting.
- **Status mapping**: status errors map 401/403 (and 404 when no token was sent to GitHub) to `source_error` with details `provider`, `http_status` and `token_sent`.

### 4. Git authentication without argv exposure
- **Where the token goes**: `Git::command` gains an optional credential. When present, it sets `GIT_CONFIG_COUNT=1`, `GIT_CONFIG_KEY_0=http.<scheme>://<host>/.extraHeader` and `GIT_CONFIG_VALUE_0=Authorization: Basic base64(<user>:<token>)`. These are applied after the existing removal of ambient `GIT_CONFIG*`, so the token is in the child environment, not in its arguments.
- **Scope**: the header is limited to the origin host. Submodules on other hosts never receive it.
- **LFS**: Git LFS honors `http.extraHeader`.
- **Usernames**: `x-access-token` for `github_repo`, `oauth2` for `git_repo`.
- **`git_repo` with a `token_file` and a non-HTTP(S) origin** is rejected at registration.

*Alternative*: embedding the token in the clone URL. Rejected because it persists in the bare repository's `remote.origin.url` and breaks the existing origin-equality check.

### 5. Commit resolution for `github_raw` and `github_source`
- **`ref`/`tag` selectors**: resolved with `GET /repos/{repo}/commits/{selector}` and `Accept: application/vnd.github.sha`, which returns the bare SHA. The result is stored as `Resolved::Git { commit }` in the source's discovery cache keyed by selector, so the existing 24-hour freshness and `always`/`max_age` policies apply unchanged.
- **`commit` selectors**: skip the API call.
- **Downloads**:
  - `github_raw`: `https://raw.githubusercontent.com/{repo}/{commit}/{path}`.
  - `github_source`: `GET /repos/{repo}/zipball/{commit}`.

### 6. Material kinds and acquisition
- **Material kinds**: all three new types produce `MaterialKind::File`. No new kind is needed, so the stores, manifests and update comparison already handle them.
- **Raw types**: natural format only, single file, no folder (spec: raw-file-sources).
- **`github_source`**: natural format only. Acquisition calls `extract_zip` with a new `strip_single_root` flag that requires exactly one top-level directory and maps folder selection beneath it. A downloaded snapshot ZIP is not useful as raw output, so `zip` format isn't offered.
- **`git_raw`**: has no commit resolution, so it behaves like a URL source. Discovery issues a HEAD for validators and records `Resolved::Url` with the platform raw URL.

### 7. Store contract
`CONTRACT` becomes `saucepan.platform-sources`. `saucepan.flat-policy-resources` joins the recognized-but-unsupported contracts with fresh-root guidance (spec: store-layout). No decoder for `git` records is kept.

### 8. Test seams
- `AcquireOptions` gains `test_gh: Option<PathBuf>` (fake `gh` executable) and `test_github_raw: Option<String>`.
- It also gains a loopback-only switch that allows `http://127.0.0.1` hosts for `git_raw`.
- All of these follow the existing `test_github_api` rule: loopback only.

## Risks / Trade-offs

- **[Gitea may reject `oauth2:<token>` Basic auth over Git HTTPS]** → Verify against a Gitea instance during implementation. If it fails, treat the username as the open question below; the change can still ship with GitLab verified.
- **[Old binary self-updates into a root the new binary can't open]** → `0.11.x` self-update installs the new binary into the old-contract managed root, which it then rejects with `unsupported_store`. Release notes and `docs/managed-setup.md` instruct reinstalling with the install script into a fresh root, and the error message names that command.
- **[`gh` adds latency to every GitHub operation]** → A single bounded call per provider operation. Cached-discovery hits don't resolve credentials at all, because the credential is only resolved when a provider request is about to be made.
- **[GitHub snapshot archives are not byte-stable]** → Provenance is the commit, and output identity comes from the extracted content manifest, not the archive digest.
- **[Snapshots omit submodules and LFS objects and reject symlinks]** → Documented limits (spec: source-archives). Use `github_repo` for full repositories.

## Migration Plan

1. Ship as a breaking minor release (`0.12.0`) with paired SDK model updates.
2. Users create a fresh root, or reinstall the managed setup, and re-register sources as `git_repo` or `github_repo`.
3. Powerspec ships a paired release registering `github_repo`/`git_repo`.
4. Rollback means reinstalling `0.11.x` against its original root, which this release never modifies.

## Open Questions

- ~~Whether Gitea needs a configurable Basic-auth username for `git_repo` token clones.~~ Resolved: Gitea accepts `oauth2:<token>` (verified in task 3.3). GitLab remains unverified.
