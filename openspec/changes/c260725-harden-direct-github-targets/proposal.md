## Why

Saucepan already accepts GitHub repositories directly, but its documented shorthand, installation identity, authentication behavior, failure classification, revision handling, and removal lifecycle are inconsistent or incomplete. Tightening this path makes direct GitHub targets dependable for users and middleware callers without coupling installation to marketplace discovery or broadening Saucepan into a general GitHub package manager.

## What Changes

- Normalize strict `owner/repo` targets to a GitHub clone URL when the configured backend is `git`, while passing explicit URLs and filesystem paths through unchanged.
- Keep the manifest `name` as the installed sauce identity and the repository as provenance; allow replacement only when the source type and origin match, otherwise return a conflict.
- Add an optional, backward-compatible `--ref` to GitHub installation, record the requested ref and resolved commit, and preserve current default-branch behavior when it is omitted.
- Delegate private-repository authentication to native Git credentials or `GITHUB_TOKEN` with `gh`; when `token` is configured with `git`, warn and continue with native Git authentication.
- Enforce the documented configuration, not-found, source, and conflict exit-code categories while retaining source fallback and compact failure context.
- Add `uninstall <manifest-name>` to remove an installed record and only Saucepan-managed GitHub or custom-Git checkouts; local source paths are never deleted.
- Keep manifests at repository root and defer monorepo subdirectories, GitHub Release assets, generalized target parsing, storage-key redesign, transactional clone staging, cache pruning, and purge workflows.
- Preserve existing command forms and index readability; new revision fields are optional, and the only intentional behavior tightening is rejecting a different origin that silently reused an installed manifest name.

## Capabilities

### New Capabilities

- `github-targets`: Direct GitHub target normalization, optional revision selection and provenance, native authentication behavior, and repository-root manifest boundaries.
- `installation-contract`: Manifest-name identity, origin-aware replacement conflicts, backward-compatible index behavior, source fallback, and stable error categories.
- `sauce-uninstall`: Safe removal of installed records and Saucepan-managed checkouts without deleting local source paths.

### Modified Capabilities

None. This repository has no existing OpenSpec capability specifications.

## Impact

- CLI parsing and dispatch in `src/main.rs`.
- Git fetch, checkout, authentication, and revision resolution in `src/sources/git.rs`.
- Install, update, and new uninstall command behavior under `src/commands/`.
- GitHub entry serialization, identity checks, and managed artifact paths in `src/index.rs`.
- BDD scenarios, integration/unit tests, and README command/configuration documentation.
- No new runtime dependency is expected.

## Governance Provenance
- Decision branch: `main`
- Decision ref: `refs/heads/main`
- Intended base: `main`
