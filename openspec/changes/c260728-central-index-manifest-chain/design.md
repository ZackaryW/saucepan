## Context

`fetch_sauce` in `src/sources/git.rs` clones or updates a target and then performs a single check: if the configured manifest file is absent from the working copy, it returns `NotFound`. That single check is the entire manifest policy, and `installation-contract` makes it normative rather than incidental.

Buckets already model "describing repositories you do not control," but two limitations keep them from helping. `BucketStub` in `src/bucket.rs` is a strict three-field struct with no flattened remainder — unlike `Sauce`, which does flatten unknown fields and therefore already carries arbitrary data into `.saucepan/index.json`. And `fetch_bucket` explicitly rejects `http://` and `https://`, so an index can only live on local disk.

The driving consumer is the `zush` gh-first migration, which needs to install plain script repositories nobody authored as sauces, and wants to carry command trees in manifest extras. But the capability is general: curating manifests for third-party repositories is useful to any saucepan consumer.

## Goals / Non-Goals

**Goals:**

- Let a manifest come from a curated index when the repository does not carry one.
- Change no behavior that currently succeeds.
- Add no new dependency, in particular no HTTP client.
- Make index-asserted versions incapable of drifting from the code they describe.
- Leave room for a further resolution link without a later breaking change.

**Non-Goals:**

- A generator hook that executes an external program against a freshly cloned target. It is a legitimate third link and the chain is shaped to accept one, but it is a materially larger trust and contract surface and is not in this change.
- Hosting, publishing, or generating indexes. Saucepan consumes them.
- Relaxing manifest requirements for targets that do carry a manifest.
- Changing bucket search semantics beyond what widened entries make visible.

## Decisions

### An ordered chain, not a boolean fallback

Replacing the existence check with an ordered list of links — rather than a special-cased "try the index if the file is missing" branch — is what makes the future generator link additive. Precedence is fixed and explicit: a repository always wins for its own identity, because the alternative would let a third party's index redefine a project that already declares itself.

### A central index is a repository target, not a URL

This is the decision that avoids a new dependency. Fetching an index over HTTP would mean adding `reqwest` or `ureq` to a binary whose network I/O is currently entirely `git` and `gh` subprocesses, and would require separately solving authentication for private indexes.

*Alternatives considered.* Shelling out to `gh api` for raw file content keeps the dependency profile and inherits auth, but only works for GitHub-hosted indexes and adds a second, differently-shaped fetch path. Adding an HTTP client is the most general but pays a dependency and an auth design for a case the existing machinery already covers.

Treating an index as a repository reuses clone, pull, ref checkout, and authentication verbatim, and yields two properties a URL fetch cannot: the index is pinnable, and its resolved commit is recordable — so "which index state produced this manifest" stays answerable.

### Required fields stay required

Widening `BucketStub` with a flattened remainder is backward-compatible when reading old indexes, but the compatibility that matters runs the other way: an older binary reading a newer index. Since `name`, `version`, and `url` are non-`Option` fields, dropping any of them would make new indexes unparseable by existing binaries. Keeping all three mandatory is therefore a normative requirement, not a style choice, and it costs nothing because a manifest-bearing entry has all three anyway.

### A manifest-supplying entry must pin a ref

This is the design's answer to drift, and it is the reason drift does not appear in the risk list as a tolerated condition. Today `sauce.version` and the checked-out tree come from one commit and cannot disagree. An index-supplied manifest could assert a version while `resolved_commit` truthfully records a HEAD that has moved on, so `list` would report a version the code no longer matches.

Requiring the entry to carry the ref it describes, and checking that ref out, makes version and tree agree by construction. The weakness becomes the feature: a central index is a pinning mechanism, which is what curating third-party repositories actually calls for.

### Unreachable indexes warn rather than fail

An index is auxiliary. Failing an installation because an unrelated registered index is unreachable would make the feature a liability for anyone who registers more than one. Warning and skipping keeps the chain's failure surface identical to today's.

### Precedence among indexes is registration order

Deterministic and explainable without new configuration. Reporting the shadowed entry keeps a silent surprise from becoming a debugging session.

## Risks / Trade-offs

- **Whoever controls an index controls installed identity for repositories they do not own.** → Record manifest source on every entry so an identity can always be traced; the existing origin-conflict rule already prevents an index from stealing a name already installed from elsewhere.
- **Exit-code feature detection changes.** Anything treating exit 1 as "this target has no manifest" now sees success where an index describes the target. → This is the only observable regression surface and is called out in the proposal; the failure category itself is unchanged when the chain is exhausted.
- **Registering an index clones a repository to read one file.** → Fetch shallowly and rely on the existing cache and pull path, which is what every other target already does.
- **A widened stub could tempt callers to treat indexes as authoritative state.** → Keep the entry's role explicit: an index supplies a manifest and a ref, and never overrides a repository that carries its own.
- **Two chain links mean two code paths to keep behaviorally identical for succeeding installs.** → Assert it directly with a test that an existing manifest-bearing install produces an unchanged entry, identity, and resolved commit.

## Migration Plan

Existing workspaces need no migration. With no index registered, the chain has one effective link and behavior is identical to today. Registering an index is opt-in, and adding entries to an index never changes how already-installed sauces resolve.

Rollback is removing the registered index: targets that depended on index-supplied manifests return to failing exactly as they do today, and no installed state is invalidated.

## Open Questions

- Whether index registration belongs alongside the existing bucket registry or in a distinct registry, given that a manifest-supplying index and a marketplace stub list are now the same artifact type used two ways.
- Whether `update` on an index-sourced sauce should re-read the index for a newer pinned ref, or hold the ref recorded at install until explicitly changed.
- Whether the shadowed-entry report on index collision should be a warning on every resolution or surfaced only through the state-reading commands.
