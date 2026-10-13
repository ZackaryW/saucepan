# Composition verification — 2026-10-13

This records `compose-retrieval-and-acquisition` on Windows x86_64 with Rust
1.98.1, using disposable fixtures and locally built manager/shim binaries.
The [composition guide](composition.md) describes the public contracts.

## Executed checks

| Check | Result |
| --- | --- |
| `cargo test --locked --all-targets` | 217 top-level test entries passed; 4 optional native-keyring tests ignored |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `openspec validate --all --strict` | 19 items passed: 18 main specs and the active change |
| Updated documentation links and JSON examples | Passed local link checks and `composition_contracts` |

Counts include ordinary fixture entry points, but exclude additional child-test
replays printed in the log. The suite covers acquisition, claims, context/source
queries, recovery, lifecycle, managed shims, retrieval and portable utilities.
The optional OS credential-manager tests were not selected. Protected metadata
journal cases use the existing isolated test backend; they do not establish
native credential-service availability.

## Combined behavior

`tests/acquisition_updates.rs` runs a traced Git flow in its own process. Initial
cached retrieval fetches once; two offline subpath copies add no fetch. Starting
an automatic acquisition performs its initial check. A call before its 60-second
interval adds no fetch. At expiry inside the source's 24-hour TTL, one fetch checks
an unrelated commit and reuses the original selected bytes/provenance. The next
due check fetches once and publishes changed selected files. No-history cleanup
defers the old leased output, then reclaims it after release. The test observes
four fetch invocations across that full sequence and checks selected versus
checked commits. A separate owned-refresh test verifies source-guard release
before updating while a prior output remains leased.

`tests/composition_cli.rs` runs ZIP and Git source and acquired consumers with
HOME/USERPROFILE removed, populated managed caches, and both temporary/no-store
retrieval. Fresh files reflect changed origins; snapshots show no managed-cache
writes. Missing origins fail instead of falling back to those caches. Children
receive original archives/bare repositories or copied subpaths as requested.
Windows inherited-handle tests cover literal arguments, streams, exit codes,
wrapper death, deferred source refresh/purge, closed-marker cleanup and failed
launches. Caller receipts cannot manufacture managed ownership.

`tests/composition_claims.rs` verifies scoped and unclaimed permissions, separate
check state, automatic ensure identity, concurrent requests, revocation during
fetch, missing covered folders and pending recovery. Invalid destination/policy
combinations leave metadata unchanged; registered caller receipts cannot
reclassify files inside their store as caller-owned inputs.

## Limits and history

Linux/macOS runtime behavior and Rust 1.89 execution were not verified here.
No hosted workflows, release publication, real Power Eagle adapter changes or
historical `mock/` mutations were performed. This does not establish a Power Eagle
IPC lease bridge or old-store compatibility.

One earlier parallel Git-heavy test invocation stalled and was terminated.
Its isolated, serial, subsequent parallel and final all-target runs passed; the
original stall remains undiagnosed. An initial full-suite attempt also caught a
documentation test that still assumed two JSON examples. The test now validates
all four documented examples, and the final full suite passed.

Implementation was checkpointed incrementally. Active planning artifacts remain
outside implementation commits, pending the separate sync/archive workflow.
