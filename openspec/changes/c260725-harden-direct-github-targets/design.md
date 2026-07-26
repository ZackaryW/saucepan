## Context

Saucepan currently treats the install argument as both a source target and a storage key, clones or pulls through `git`/`gh`, reads one root manifest, and indexes the result by `sauce.name`. The direct-GitHub path is small and useful, but several contracts do not line up: raw Git does not expand `owner/repo`, GitHub entries do not retain revision provenance, same-name entries from different repositories can replace one another, token handling differs by backend, source errors collapse into not-found, and the conflict remediation names an absent `uninstall` command.

The implementation must remain compatible with existing `saucepan.toml` files, command forms, and index entries. The repository has no prior OpenSpec baseline, so the accompanying capability specs establish the initial normative behavior.

## Goals / Non-Goals

**Goals:**

- Make direct GitHub targets work consistently with both configured backends.
- Preserve manifest-name identity while making repository provenance and revision state explicit.
- Support optional Git refs without changing default-branch installs.
- Use native authentication mechanisms and expose accurate, stable error categories.
- Provide the minimal removal operation required by origin conflicts.
- Keep existing indexes readable without a migration command.

**Non-Goals:**

- Marketplace-driven installation changes.
- Monorepo subdirectories, GitHub Release assets, generalized target parsing, or a new source abstraction.
- Credential storage, OAuth, generated askpass helpers, or global Git configuration changes.
- Transactional clone staging, rollback orchestration, purge, pruning, or cache management.
- Deleting paths belonging to local sources.

## Decisions

### Normalize only strict GitHub slugs for raw Git

A small helper will recognize exactly two non-empty forward-slash components with no URL scheme, drive prefix, backslash, or additional slash. When the backend is `git`, that form becomes `https://github.com/<owner>/<repo>.git` for cloning. Explicit URLs and filesystem paths pass through unchanged, and the logical target stored in the index remains the caller-provided repository identity.

This fixes the documented shorthand without introducing a URL parser or changing custom-Git behavior. The existing readable `repo_dir()` mapping remains the storage rule.

### Keep manifest name as identity and repository as provenance

`IndexEntry::name()` remains the lookup key for `path`, `update`, `cat sauce`, and `uninstall`. Replacement is allowed only when the existing and incoming entries have the same source variant and origin: path for local, repository for GitHub, and URL for custom Git. A different origin claiming the same manifest name returns `Conflict`, including when both origins use the same source type.

This preserves existing middleware and index semantics while eliminating silent replacement. No alias or compound identifier layer is introduced.

### Add revision metadata only to GitHub entries

The install command gains optional `--ref <value>`. GitHub index entries gain optional `reference` and `resolved_commit` fields with Serde defaults so old entries continue to load. `resolved_commit` is refreshed after every successful GitHub install or update; `reference` is absent when the default branch is used.

For a requested ref, Git fetches current remote refs and resolves in this order: matching remote branch, matching tag or other named ref, then commit expression. The checkout is detached at the resolved commit. Updating repeats resolution of the stored ref, so branches advance, ordinary tags continue to resolve to the same tagged commit, and commit SHAs remain pinned. Without a requested ref, existing default-branch pull behavior remains and only the resolved commit metadata is added.

An old binary may ignore and later discard the optional fields when rewriting an index. Avoiding that downgrade limitation would require a separate metadata store and is not justified.

### Delegate authentication to the selected native backend

For `gh`, Saucepan passes the configured token through `GITHUB_TOKEN`; Git commands operating on a clone created by `gh` also inherit it so a configured gh credential helper can use it. For raw `git`, Saucepan relies on the user's credential helper, SSH agent, or existing Git configuration. If a token is configured with raw Git, Saucepan emits one warning per operation and does not synthesize credentials. The ineffective `GIT_USERNAME` and `GIT_PASSWORD` variables are removed. Existing SSL-key forwarding remains unchanged.

This avoids handling secret files or embedding credentials in URLs or process arguments.

### Return typed source outcomes through fallback

Git/gh launch, authentication, network, fetch, checkout, and pull failures become `SourceError`. A known-absent local target or missing manifest is `NotFound`; a remote clone failure remains `SourceError` when Git cannot reliably distinguish absence from authentication or transport failure. No enabled sources is `ConfigError`.

Install continues trying enabled sources in precedence order. It retains compact per-source context; if all attempts miss it returns `NotFound`, while any source failure makes the final result `SourceError`. Messages retain `could not install '<target>'` so existing human-facing assertions and diagnostics remain useful.

### Add bounded uninstall behavior

`uninstall <manifest-name>` locates the entry by existing manifest identity. Local entries are removed only from the index. GitHub and custom-Git entries resolve their checkout with `artifact_path()`, verify that the lexical target stays beneath the expected `<root>/github` or `<root>/customgit` directory, remove the managed checkout if present, then remove and atomically save the index entry. An unknown name returns `NotFound`.

The command has no purge, keep-files, orphan discovery, or transactional rollback modes.

## Risks / Trade-offs

- **[Ref names can be ambiguous]** → Use a documented resolution order and store both the requested ref and resolved commit.
- **[Old binaries discard revision metadata on index rewrite]** → Keep fields optional and treat downgrade metadata loss as a documented limitation; core install identity remains intact.
- **[Raw Git token configuration previously appeared supported]** → Continue accepting the field, warn clearly, and use native credentials instead of failing existing configurations.
- **[Git cannot reliably classify every remote absence]** → Classify ambiguous process failures as `SourceError` rather than misreporting authentication or network problems as absence.
- **[Uninstall deletes managed working trees]** → Restrict deletion by source type and containment; never delete local entry paths.
- **[Deletion and index save are not transactional]** → Keep the operation minimal and return failures; transactional staging remains explicitly out of scope.

## Migration Plan

1. Add optional GitHub index fields and compatibility tests before changing command behavior.
2. Introduce target normalization, ref resolution, typed source outcomes, and origin-aware conflicts behind existing command forms.
3. Add the `uninstall` command and managed-path safety tests.
4. Update BDD scenarios and README examples, then run the full unit and integration suite.

Rollback is code-only: existing index entries remain valid because new fields are optional. Indexes written with revision fields remain readable by older binaries, subject to the documented loss of those fields when an older binary rewrites the file.

## Open Questions

None. The owner confirmed the capability boundaries and compatibility trade-offs during clarification.
