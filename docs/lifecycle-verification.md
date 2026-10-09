# Application lifecycle verification

This is historical checkpoint evidence. Its earlier record layout and scripts do
not define current schema compatibility. Use [claim registry verification](claim-verification.md)
for the current implementation and runnable acceptance commands.

Verified on Windows x86_64 on 2026-10-08. The coordinated change covers managed
install, update, rollback, launcher ownership/maintenance, uninstall and explicit
reconciliation. Earlier changes remain unarchived; planning artifacts are kept
outside implementation commits.

## Executed checks

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
uv run --no-project python scripts/verify_windows_console.py
uv run --no-project python scripts/access_acceptance.py
uv run --no-project python scripts/lifecycle_acceptance.py
openspec validate reconcile-application-lifecycle --strict
```

All checks passed. The all-target suite contains 82 passing tests, including
child-fixture entry points. The hidden-console recipe separately confirms both
CTRL_BREAK forwarding through the managed shim and interruption of leased `run`
consumers; console-dependent tests alone cannot establish that from a detached
test invocation.

Linux/macOS execution, Unix exec/signal behavior and the declared Rust 1.89
minimum toolchain were not verified on this host. No runtime parity or universal
filesystem power-loss durability is claimed. Recovery tests construct interrupted
durable states and use deterministic barriers; they do not simulate hardware
power loss.

## Integrated release run

`scripts/lifecycle_acceptance.py` completed 29 recorded CLI operations under
`target/lifecycle-acceptance/6dc381d5-216c-4271-bc8e-68d5778ba7cc/root`.
Its sibling `report.json` contains the full results, operation IDs, claim/binding
snapshots, owned file identities and final retention outcomes.
`target/lifecycle-acceptance/latest.json` points to that report.

| Build | SHA-256 |
| --- | --- |
| Application A | `65d6c96b8e6347a5117dff8f865a7fe879890b99866346beb4cb9edf70403146` |
| Application B | `eb850253c33a69fd1e96aab27c426d6ab830e03c426f46e906a5dea6fbd63a50` |
| Launcher A | `2de09983364e2d25c54c27d3c75335d0b755d56def8843135c1c42e830a2372f` |
| Launcher B | `a8bb79b1c3bf3b02ad9503d8a784db559eaa85a991c4f17c471849d82d8a4fc7` |

The run bootstraps A from a loopback release ZIP, shares A with a second app,
keeps an A target alive, updates through the installed shim to B, rejects an
unusable candidate without changing the app, rolls back to A, deactivates and
reactivates, replaces the launcher after a pending result, then self-uninstalls.
It confirms the other app still launches, removes that app, releases the held
target, reconciles with a direct binary, repeats removal, and verifies ordinary
retention reclaims A while retaining B. Generic CLI install/uninstall also runs.
Release discovery creates no repository clone directory.

All 96 existing `mock` files had identical before/after digests. The recipe uses
only its unique simulated root; it does not update the installed mock manager.

The separate access report is
`target/access-acceptance/b63dfc03-15b0-4c90-a887-34b821c2c360/report.json`.
It records zero process markers after five path/verify pairs, removal of five
seeded stale markers, exact native claim access, live external consumption,
post-exit reclaim and unchanged mock launcher bytes. Its old-shim fixture starts
against an isolated old-shape app record, restores new metadata after launch,
and proves inherited output protection survives wrapper death. Old binaries
reading or writing new app metadata are not supported by this result.

## Specification scenario map

Test names below are in `tests/lifecycle.rs` unless a module/file is identified.
The real-build recipe supplements the deterministic tests with actual release
binaries and launcher contention.

| Application lifecycle scenario | Passing evidence |
| --- | --- |
| Initial candidate fails its probe | `failed_install_has_no_live_application_and_cli_install_matches_native_results`: no live app or launcher; a later CLI install succeeds |
| Installation interrupted during publication | `core::lifecycle::tests::durable_candidate_survives_reclaim_and_invalid_candidate_releases_protection`: before either file, after sidecar, after both; same app/claim IDs, no app before recovery |
| Failed self-update preserves both selections | `managed_claim_and_binding_commit_together_and_rollback_preserves_request`; real-build failed update compares the complete app record |
| Rollback selects a retained output | Same managed test preserves the request; real-build rollback uses the cached A output after release B |
| Invalid rollback target | `managed_update_rejects_aba_and_foreign_rollback` rejects a different item without moving the binding; full candidate integrity is shared with recovery and activation checks |
| Reclaim races candidate preparation | `core::store::access::tests::activation_scans_once_per_phase_outside_mutation_lock` observes held entry protection during both scans; durable-candidate test acquires a newer output with keep-one while intent pins the old output |
| Committed install intent survives process exit | Durable-candidate test drops in-memory guards, reopens, verifies pinned intent and completes; corrupt/missing payloads instead fail and release protection, cleaning partial launcher files |
| Deactivate and later reactivate | `tests/managed.rs::deactivation_preserves_live_targets_claims_and_other_bindings`; real-build deactivate/reactivate uses the same launcher |
| Deactivation overtakes managed update | `managed_update_detects_deactivation_during_probe` uses a probe barrier and checks conflict plus unchanged claim; ABA test also changes settings away and back |
| Uninstall overtakes candidate probing | `uninstall_fences_both_managed_and_binding_only_inflight_probes`; `core::lifecycle::tests::uninstall_fences_pending_maintenance_and_claim_publication` covers delayed maintenance/claim publication |
| Self-uninstall through a busy Windows launcher | Real-build self-uninstall returns pending; direct-binary reconciliation after exit completes |
| Shared outputs outlive registration | `uninstall_preserves_shared_outputs_and_live_target_then_reconciles`; real-build second-app launch and held target remain usable |
| Retention remains independent | Same shared-output test and `uninstall_fences_pending_maintenance_and_claim_publication` preserve surviving output-policy demand; later ordinary reclaim handles eligible entries |

| Recovery scenario | Passing evidence |
| --- | --- |
| Original response is lost | Managed install replay returns the original app; shim replay preserves revision; uninstall replay returns the original operation |
| Retry changes shim/application inputs | `core::lifecycle::tests::operation_identity_and_pending_work_are_isolated` rejects changed fingerprint before effects and verifies same-operation locking |
| Restart during record publication | `lifecycle_journal_couples_application_and_receipt_at_each_record_boundary` and `lifecycle_journal_replays_records_retirement_and_receipt`, both in `core::lifecycle::tests`, cover app/receipt writes and directory retirement |
| Restart before validation finishes | `interrupted_preparation_rechecks_probe_without_publishing_a_transition`: reopen/reconcile stays preparing; original-command retry reruns the failing probe and preserves the app |
| Busy cleanup does not poison store | Shared-target test and changed/unknown-file test register/use unrelated apps while cleanup is pending |
| Reconciliation while consumer holds a resource | Shared-target test and integrated release recipe preserve inherited output leases across logical removal and subsequent reclaim |
| Complete simulated lifecycle finishes | The 29-operation release report above records digests, IDs, transitions, cleanup, retention and unchanged mock contents |

| Shim maintenance scenario | Passing evidence |
| --- | --- |
| Two apps request colliding names | `core::shims::tests::concurrent_names_have_one_owner`; collision test covers executable case and sidecar aliases |
| Only configuration path occupied | `shim_collisions_and_replacement_conflicts_preserve_existing_files` preserves an orphan unowned sidecar |
| Replacement encounters a running launcher | `tests/managed.rs::deactivation_preserves_live_targets_claims_and_other_bindings` plus real-build replacement returns pending on Windows, then reconciles |
| Existing launcher externally changed | Shim collision/replacement test leaves changed bytes and app revision untouched; restoration allows retry |
| Configuration changes between removal attempts | `uninstall_preserves_changed_and_unknown_files_without_blocking_other_apps` changes sidecar between attempts; both pair files remain until identity is restored |
| Target remains alive after launcher cleanup | Native/process access regressions and real-build lifecycle separate output lease lifetime from registration/launcher lifetime; Windows also verifies deferred launcher cleanup |
| Adopt copied existing shim | `core::shims::tests::adoption_is_explicit_and_missing_owned_pair_can_be_restored` verifies exact existing pair, adoption and restoration of missing owned files |
| Explicitly leave unmanaged shim | `unmanaged_shims_require_explicit_adoption_or_exclusion` first rejects removal, then returns the unmanaged leftover without deleting it |

Shim file-boundary replay is additionally covered by
`core::shims::tests::shim_publication_recovers_each_file_boundary_and_retry_is_idempotent`.
`tests/cli.rs::lifecycle_inspection_reports_pending_without_success_exit` checks
inspection/reconcile output and nonzero pending status. Portable path/tree
fixtures retain link/reparse rejection coverage.

## Reconciliation with earlier contracts

| Established behavior | Result |
| --- | --- |
| Stable launcher; downloads do not activate | Managed updates change claim/binding after validation; the release recipe separately replaces the shim |
| Failed activation retains usable state | Managed transitions additionally preserve the selected claim; primitive activation remains binding-only |
| Metadata-only routine launch; explicit full verification | Observer tests and release access acceptance retain zero payload IO on lookup/launch and explicit corruption detection |
| Claims are not pins; consumers own leases | Access/run/managed tests and shared-output removal retain this distinction |
| Only process launches create lifetime markers | Native access leaves zero markers; reclaim sweeps stale markers without discarding live target protection |
| Deactivation is idempotent; later activation is allowed | Existing deactivation tests still pass; managed updates additionally reject revision conflicts |
| Source discovery, immutable output identity and retention | Git, remote, local, claims and reclaim regression suites pass; no provider/schema-version migration was added |
| Inspectable existing mock | All mock file digests unchanged; new lifecycle artifacts live in the isolated acceptance root |

Implementation commits retain ordinary incremental boundaries: operation
contracts, shim ownership, managed transitions, uninstall, then integrated
verification. No dependency additions, production-root changes, migration
decoder, background cleanup service or automatic archival were introduced.
