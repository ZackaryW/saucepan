## Context

See `proposal.md` for the motivating remote failure. The current Git exporter reads committed objects, expands exact submodules and LFS content, and rejects mode `120000`. The resulting ordinary tree feeds existing manifest hashing, ZIP writing, and central publication. Folder selection happens after full-source preparation. Generic filesystem copying and ZIP extraction currently reject links; those boundaries remain intact.

## Goals / Non-Goals

**Goals:** Resolve Git aliases deterministically without consulting host filesystem links, preserve executable and dependency evidence, and keep the existing core/store/SDK flow. Helpers should be small, generic where useful, and developed with failing tests first.

**Non-Goals:** No OS link creation or privileges; no local/URL provider link support; no new app permission model, source identities, installation units, schema version, settings, or CLI/SDK switches. Do not revive the rejected code in `src2`, `src3`, or the legacy branch. Do not introduce a general recovery journal or rewrite unrelated acquisition components.

## Decisions

### Resolve a logical committed tree before writing aliases

Separate Git object discovery from ordinary-tree emission. Discover regular files, directories, link blobs, and included submodule trees under one logical source root, retaining the owning repository, exact revision, blob/mode, and existing dependency evidence. Resolve links through this inventory. Emit independent ordinary files/directories into the existing temporary export, then use the existing publication flow.

Keeping link resolution out of the host filesystem prevents absolute-path access, privilege requirements, and platform-specific link behavior. Native checkout-and-follow was rejected because it introduces those dependencies. Reinterpreting generic filesystem copying would also broaden local-source behavior beyond this change.

The implementation stages ordinary blob payloads in a private `input` tree during discovery, resolving LFS in each original repository/revision context. The logical inventory retains directory entries, link text, and resolved file sizes; original paths connect it to those staged payloads. A separate `tree` export contains the resolved independent copies. This reuses the existing blob/LFS loader and dependency tracking without introducing a second Git reader. The extra temporary copy is bounded by the export limits and is removed with the existing temporary directory.

### Use relative path traversal with explicit link expansion

Link text is a UTF-8 relative POSIX path, based at the link's original parent. Support `.` and in-root `..`; resolve intermediate aliases before processing later components, including later `..`. Reject root escape as it occurs rather than normalizing away evidence of traversal. Reject absolute, drive-qualified, UNC, backslash, NUL, empty, and invalid UTF-8 targets. Treat `.git` administration components as inaccessible, including indirect aliases and case variants. Do not trim link text into a different target.

Use active resolution/expansion ancestry to detect cycles. Reusing a completed target through unrelated aliases is valid. A directory alias pointing to an ancestor that would recursively copy itself is a cycle. Target lookup uses original committed paths, even when content is being emitted under another alias.

A pure resolver under `src/utils/` can accept a logical-entry lookup and limits, with no Git commands, filesystem reads, or core imports. Keep Git object loading and emission inside a focused `src/core/sources/git/` component. Select concrete helper names during implementation; do not build an extensible policy framework.

### The source root is the boundary; folder selection happens afterward

An alias under `sdk/` may refer to `../shared/` because both belong to the same source snapshot. Its resolved content is copied at the alias path before `sdk/` is extracted. This does not add sibling folders to the output or create another unit/source/history queue. An alias directory can itself be selected.

The logical root includes required exact submodules. Links crossing a submodule boundary use that composed tree, without triggering additional link-directed clones or reads from other cached sources. LFS expansion and verification remain at the final target's original repository/revision. Aliases inherit validated bytes and executable metadata; dependency records remain associated with the included source inputs.

### Bound expansion and preserve failure semantics

Use at most 64 link hops per resolved output path and the existing source limits of 1,000,000 entries and 16 GiB of file bytes for the complete expanded export. Count repeated copies at alias paths toward entry/byte limits. Check budgets while resolving/emitting so cycles or fan-out do not exhaust resources before detection. Tests inject smaller internal limits instead of allocating production-size fixtures. These are implementation safety bounds, not new app settings.

All resolution failures are export failures. They must occur before publishing current/history or app touches and must never be classified as remote-unavailability fallback. Existing repository preparation can leave cached Git objects after failure; this change does not promise filesystem-wide rollback or garbage collection. General special-file rejection and output path/collision checks still apply.

### Keep ordinary snapshots and current identities

The emitted manifest contains ordinary target bytes and executable metadata at each alias path. Independent copies avoid hardlink coupling. Existing source/ref identity, commit, content digest, and dependency inputs continue to identify snapshots. No public link metadata or schema field is needed: previous versions never successfully exported committed symlinks, while link-free exports retain identical manifests and IDs.

Archive writing must not encode symlink modes or pointer text. Content verification checks the resolved copies, and mirrors remain independent. Historical retention, exact pins, app filtering, marker stability, and HMAC/index validation stay unchanged.

## Risks / Trade-offs

- Alias fan-out increases export size and work → count actual duplicated entries/bytes, enforce hop limits, and memoize resolution only where it preserves cycle detection.
- Incorrect interpretation of intermediate links and `..` could escape the boundary or copy wrong content → test component-by-component traversal, directory recursion, and cross-submodule cases against committed fixtures.
- Platform filename differences remain → retain existing destination validation and explicit failure for unrepresentable names; symlink support does not relax collision rules.
- Live GitHub content or availability changes → keep deterministic fixtures as required automated coverage and run the motivating repository at its recorded commit as a separate HTTPS acceptance check.
- Flattening removes link identity → document that outputs contain copies and that editing an alias does not update its target. Native-link preservation remains outside this change.

## Migration Plan

Implement helpers first with red/green tests, integrate Git export, then run public API/CLI/SDK regressions and the pinned remote case. No index migration or re-enrollment is needed. Update current documentation only when behavior is implemented and verified.

Deploy through the existing shared executable mechanism without changing SDK commands. Rolling back the executable restores rejection of fresh Git exports containing links; already retained ordinary ZIPs and content remain readable under the existing schema. Do not rewrite old snapshots or claim unexecuted platform checks passed.
