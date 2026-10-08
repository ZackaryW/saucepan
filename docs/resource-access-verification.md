# Resource access verification

## Application deactivation follow-up

The `deactivate-applications` change passed formatting, strict all-target Clippy,
all-target tests (60 passing, including fixture entry points), and strict
OpenSpec validation on Windows x86_64 on 2026-10-08.

- Native coverage clears an unavailable binding, preserves concurrent settings
  and existing claims, rejects invalid application IDs, and proves a repeated
  call does not rewrite the record.
- Journal fixtures recover deactivation both before and after app-record
  publication, preserving claims and settings.
- A real CLI and installed shim test confirms JSON results, continuing target
  execution, failed new launches, independent binding protection, delayed
  reclaim after exit, ordinary retention, and successful reactivation through
  the unchanged shim.

Tests used temporary stores. The existing `mock` installation was not modified.
Linux/macOS and Rust 1.89 execution remain unverified. Console signal handling
was not changed or separately rerun for this follow-up.

## Lease-marker cleanup follow-up

The `clean-lease-markers` follow-up was verified on Windows x86_64 on
2026-10-08: formatting, strict all-target Clippy, all-target tests (57 passing,
including fixture entry points), both hidden-console checks, isolated release
acceptance, and strict OpenSpec validation passed.

- `tests/access.rs::non_process_access_never_creates_lifetime_markers` failed
  before the fix with four markers in its first iteration; afterward, 20 rounds
  of native path/full/entry-lease/claim-lease access and CLI path/verify leave zero.
- `sweep_preserves_live_markers_and_creation_windows_on_retained_outputs`
  checks stale cleanup, native live handles, the protected creation window,
  retained/active output preservation, unchanged provenance, malformed-name and
  directory preservation, and stable coordination locks.
- `tests/run.rs::retained_process_marker_survives_wrapper_death_then_is_swept`
  confirms a live inherited marker remains after wrapper termination, then is
  removed after target exit while the output stays ready. Failed-run coverage
  also asserts zero stale markers after reclaim.
- Fresh release acceptance at
  `target/access-acceptance/3361f5bb-5b49-442c-9c42-0e04f1e9fdc9/report.json`
  records zero markers after five path/verify pairs and removal of five seeded
  stale markers on an active output. It verifies the previous mock launcher's
  inherited marker after wrapper death. Original mock launcher digests are
  unchanged. Application SHA-256 is
  `89033bb5c24a18e575169368777f53ea5267632fb8b4824d156cc056521774e9`;
  shim SHA-256 is
  `1885d2404230b84ecca42798d39201b4990a2bac45abbf1c108594b89af807e9`.

Cleanup runs during reclaim/acquisition, not on an idle timer. No schema,
retention rule or installed mock binary changed. Linux/macOS and Rust 1.89
execution remain unverified. The original resource-access evidence below is
historical; its binary identities predate this follow-up.

## Original resource-access change

Verified on 2026-10-08 using Windows x86_64, Rust/Cargo 1.98.1.

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo build --all-targets`, and `cargo test --all-targets` passed. The test
suite reports 54 passing tests, including child-fixture entry points. Console
tests that require an attached console were separately executed with
`uv run --no-project python scripts/verify_windows_console.py`: both the shim
CTRL_BREAK and leased-run interruption evidence markers were present.
Both `efficient-resource-access` and `reimplement-saucepan` passed strict
OpenSpec validation.

| Contract / scenario | Executed evidence |
| --- | --- |
| Metadata lookup performs zero payload traversal or reads | `core::store::access::tests::observed_io_and_paused_scan_do_not_serialize_unrelated_operations`: 0 directories, 0 opens, 0 bytes |
| Full access scans once | Same observer: 1 directory, 1 open, 7 bytes for a seven-byte payload |
| Full scans protect content while unrelated work proceeds | Paused inspector plus concurrent acquisition, metadata lookup, app registration and reclaim; unrelated work completes before resumption |
| Verification modes preserve identity, provenance and freshness | `tests/access.rs::metadata_and_full_access_native_and_cli_preserve_identity`: exact old UUID; source/index bytes unchanged |
| Same-length edits, missing/extra/type-changed payloads fail full checking | Same test through native and CLI access; no success JSON on failure; original manifest unchanged |
| Read failure and invalid mode are not downgraded | Windows sharing-denied payload fixture and invalid CLI mode in `tests/access.rs` |
| Wrong UUID/kind/state, missing/non-directory roots, forbidden links fail metadata access | `metadata_rejects_wrong_identity_kind_state_and_root`; Windows junction exercised |
| Multiple consumers defer reclaim until final release | `claim_selection_and_multiple_guards_keep_exact_output_alive`; unchanged output record before final drop |
| Reclaim wins before lease selection | Same test: leasing the reclaimed output fails without revival |
| Claim updates/releases do not redirect guards | Same test plus `simultaneous_claim_update_returns_one_consistent_selection`, with a barrier-started update and selection |
| Shim validates its active declared executable without payload IO | `managed_launch_boundary_checks_target_without_payload_io`: zero directory/open/byte events; undeclared, unsafe, missing and non-file target rejection; root forwarding |
| Activation keeps pre/post integrity checks without global scan locking | `activation_scans_once_per_phase_outside_mutation_lock`: exactly two complete scans, mutation lock available and entry lock held in both phases |
| Probe mutation preserves prior binding | `tests/managed.rs::probe_mutation_preserves_binding_and_corrupted_reuse_is_rejected` |
| Local reuse and remote cached-copy reuse still reject corruption | Managed mutation test and `tests/remote.rs::zip_integrity_tag_and_shared_platform_discovery` |
| Interrupted publication cannot accept corruption | `core::index::tests::interrupted_publication_recovers_before_and_after_rename`: intact and corrupted fixtures before/after rename; old claims preserved on failure |
| Managed replacement, rollback, streams/status, wrapper death | `tests/managed.rs::active_switch_failed_probe_rollback_streams_and_launcher_death` and hidden-console run |
| External consumer sees exact directory, literal arguments, inherited cwd/streams/status | `tests/run.rs::literal_arguments_environment_streams_cwd_and_status`; child-only environment override, exit 23, no wrapper JSON |
| Claim-independent lifetime survives wrapper death | `consumer_survives_claim_release_retention_and_wrapper_death`; Windows inherited marker remains after wrapper kill |
| Failed spawn/full verification/invalid mode/missing program leave no live ownership | `failed_launch_or_verification_does_not_leave_live_ownership`; subsequent acquisition reclaims the old entry |
| Ordinary native guard is non-inheritable | `native_guard_is_not_inherited_by_unrelated_spawn`; reclaim succeeds while unrelated child is still alive |
| Supported interruption releases native ownership | Separate hidden-console execution of `consumer_console_or_signal_interruption` |
| Actual external example survives retention and claim release | Built `examples/read-resource.rs`, run through the real CLI during acceptance, then reclaimed after exit |
| Existing launcher markers remain compatible | Acceptance copies the previous mock shim into an isolated installation, starts a target, replaces the binding, kills the wrapper and verifies the new reclaimer defers its inherited lifetime |

The integrity-call audit preserved acquisition preparation/publication manifest
checks in `core/store/mod.rs`, compatible reuse there and in
`core/source/remote.rs`, and staged recovery checks in `core/index/mod.rs`.
Only routine launch and redundant same-phase activation scans changed.

Fresh release acceptance is recorded in
`target/access-acceptance/1eaa87d1-b60f-4001-a785-9b0ecff0ef0f/report.json`
and `target/access-acceptance/latest.json`. The installed application reports
`saucepan 0.1.0+efficient-access`. SHA-256 values:

| Binary | SHA-256 |
| --- | --- |
| saucepan.exe | `14b959418323c0b3ccb64c95d50d214c14973369caff0f219fbce699b4f67d06` |
| saucepan-shim.exe | `94a29452a1ed00f2f65a173cf9f2d9b1e79cc0ab122e5634bd395fe133592a5f` |
| read-resource.exe | `0c78992cedd0d3450aebb0ad9a81197937ee0f404032eb582ffa979299ff3528` |

The acceptance logs record actual per-test inspector events, not inferred
timings or a system-wide trace of the release binary. The installed fresh shim
matches the built shim digest. Existing mock launcher files have identical
before/after digests; all mutations used explicit isolated roots.

Linux/macOS runtime behavior, Unix signal/exec tests and executable-permission
tampering, and the declared Rust 1.89 minimum toolchain were not executed on
this host. The earlier Linux cross-check was blocked by the missing
`x86_64-linux-musl-gcc` compiler. No cross-platform parity is claimed.

Final diff review found no changes to persisted requests, source/index/policy
schemas, source providers, dependencies, retention rules, SDKs, or production
root selection. The intentional behavior change is metadata-default access;
full verification remains explicit and available. Planning artifacts stay local
under `openspec/changes`; the original baseline and this change are not archived.
