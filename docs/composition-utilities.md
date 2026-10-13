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

## Crate reuse decisions

Reviewed on 2026-10-13 during the flat-policy implementation. These decisions
distinguish shipped changes from candidates that still need evaluation. A crate
must reduce maintenance while preserving the contracts above; familiarity alone
is not a reason to add a dependency.

| Package or facility | Decision | Reason and boundary |
| --- | --- | --- |
| RustCrypto `sha2` | Consolidated on the already used `0.11.0`; removed the separate `0.10` dependency and `sha2_crypto` alias. | Content hashes, metadata checksums and HMAC/HKDF now share one SHA-256 implementation. Six obsolete package versions disappear from the lockfile. Digest bytes and record framing remain unchanged. |
| `hex` | Added as a direct dependency, using the already locked `0.4.3`. | `sha2` 0.11's output array does not implement the old `LowerHex` formatting API. `hex::encode` supplies lowercase digest encoding without a custom encoder; it also replaces the test-only hand-written formatter. This is encoding, not cryptography. |
| `walkdir` | Deferred; not added. | It could replace recursive directory enumeration, but our walker also rejects case collisions and reparse points, bounds entries/bytes, checks source changes, and preserves executable metadata. Keep these rules even if traversal moves to a crate. Reconsider with equivalent tree/link/limit tests and a measured reduction in traversal code; explicitly disable root-link following as well as child-link following. |
| `dunce` | Deferred; not added. | It is a candidate for the small Windows `git_path` helper. It preserves verbatim paths when simplification cannot safely represent them, so it is not an assumed drop-in replacement for the current Git-specific spelling. Reconsider after checking long paths, UNC paths and Git/LFS command behavior on Windows. |
| Rust standard file locks | Retained; no extra locking crate. | The project already requires Rust 1.89, which supplies these APIs. Our wrapper adds the required deadline and stable lock-file lifecycle; another locking package would not remove those responsibilities. |
| `tempfile`, `process-wrap` | Retained. | These already own temporary-file cleanup and native job/process-group mechanics. Saucepan retains publication coordination, bounded capture and deadlines because those are its caller-visible contracts. |
| `serde`, `serde_json`, `uuid`, `globset`, `url`, `ureq`, `zip`, RustCrypto/keyring crates | Retained for their existing responsibilities. | Parsing, identity, matching, transport, archive decoding and cryptographic primitives already use libraries. No replacement was demonstrated by this review. Domain limits, verification and optional custody remain explicit wrappers. |
| General policy/graph engine | Not introduced for this change. | Flat leaf definitions need bounded UUID lookup and Saucepan-specific precedence, constraints and revision checks. A new engine would still need those integration rules and would broaden the policy contract. Reconsider only if the policy language gains requirements that justify it. |

Upstream references: [SHA-256 API](https://docs.rs/sha2/0.11.0/sha2/trait.Digest.html),
[hex encoding](https://docs.rs/hex/0.4.3/hex/fn.encode.html),
[walkdir traversal controls](https://docs.rs/walkdir/2.5.0/walkdir/struct.WalkDir.html),
[dunce path simplification](https://docs.rs/dunce/1.0.5/dunce/fn.simplified.html),
[standard file locks](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock),
[tempfile publication](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html),
and [process-wrap](https://docs.rs/process-wrap/latest/process_wrap/).

The SHA-256/hex checkpoint passed 43 focused crypto, utility, metadata-integrity
and policy-catalog tests on Windows, plus formatting and all-target Clippy. One
explicit native-keyring integration test was ignored. Dependency resolution used
the local Cargo cache after the registry hostname failed to resolve. This evidence
does not establish Linux/macOS runtime behavior or complete the flat-policy change.
