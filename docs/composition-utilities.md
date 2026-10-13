# Composition utility contracts

These helpers contain no store identities or application policies. Callers select
their paths, coordinate access and decide whether returned bytes are publishable.

- `transfer::copy_bounded` copies a reader to a writer, reports byte count and
  optionally computes SHA-256. It reads at most one byte beyond the limit and never
  writes beyond it. A failure may leave a partial writer; use private staging and
  publish only after success. A computed digest is not expected-digest verification.
- `Git::hydrate` ensures a pinned commit's submodule/LFS dependencies are present.
  `Git::export_offline` consumes that material without fetching missing objects or
  writing dependency caches. Both accept explicit repository, dependency and
  scratch paths. Offline export fails on missing or corrupt dependencies. The
  existing `Git::export` remains the fetch-and-export convenience operation.
- `tree::copy_tree_unhashed` copies files with the same limits, path checks and
  executable metadata as `copy_tree`, but returns no manifest or file hashes.
  Both require a stable source and a new private destination. Use staged delivery
  when partial copies must be cleaned up on failure.
- `lifetime::inheritable_marker` and `marker_held` expose native handle liveness
  without PID or age guesses. The caller must serialize marker creation and
  sweeping and preserve inherited handles; the helpers do not decide when to
  delete application data.
- `path::new_destination` resolves an absent name beneath an existing parent
  without reserving it. Publication still requires create-new coordination.
- `path::disjoint_destination` rejects occupied or overlapping destinations and
  resolves a new destination beneath its existing parent before effects.
- `fs::staged_directory_coordinated` serializes cooperating publishers through a
  caller-supplied native lock outside the destination. The lock must have a stable
  identity and must not be unlinked after publication. The parent is trusted;
  this does not promise protection against arbitrary external filesystem writers.

`cargo test --test utilities` checks limits, failed publication, optional digest
results, cross-process delivery, and offline Git exports with submodule/LFS
origins removed. Compilation alone does not verify another host's native behavior.
