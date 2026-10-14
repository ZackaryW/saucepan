# Group-lease verification — 2026-10-13

Executed on Windows x86_64 (`x86_64-pc-windows-msvc`, Rust 1.98.1).
The `add-group-leases` change extends the existing flat policy contract without
new dependencies, persistent records, migrations or compatibility adapters.

## Verified behavior

- Strict named requests reject malformed targets, unknown fields, duplicate names,
  unsafe paths, invalid member counts and reference limits. Documentation JSON is
  parsed by contract tests. The maximum 128-member request is accepted.
- Group access policies intersect existing authority. App denial remains denial;
  trusted local access still obeys a supplied boundary. Direct outputs cannot
  borrow claim scope from another member. Catalog shadowing is rejected.
- Verification is a floor, duplicate-output views share one guard and scan, and
  metadata-only groups read no payload bytes. Failure on a later verified output
  releases every guard. Directory views are validated before protection begins.
- Relevant updates, release, uninstall, credential changes and source/app policy
  adoption during scanning reject admission. Unrelated catalog writes and reads
  proceed; reversed-order groups and reclaim remain usable during a paused scan.
- Native groups pin old outputs independently, create no process markers, have no
  metadata publication or retrieval effects, and are not inherited by unrelated
  children. Source input can be removed before leasing its existing output.
- Real isolated processes cover partial inheritance, failed spawn, literal
  arguments, streams, Unicode/space paths, working directory, child status and
  Windows console interruption. CLI admission errors have stable codes, no partial
  stdout and no consumer start. Both request and catalog byte limits are tested.
- A Zush-shaped fixture retains three outputs for command, hooks and service,
  with a fourth label for a contained view. Both unrestricted and app-bound flows
  survive claim updates, release and wrapper termination. Reclaim preserves every
  live output; the descriptor remains readable until the child exits. Later
  consumer cleanup removes the closed owned descriptor workspace.

The fixture is `tests/group_leases.rs`; unit admission tests are in
`src/core/lease/group/tests.rs`. Existing single-resource, claim, temporary-source,
temporary-output and managed shim/lifecycle regressions were also executed.

## Executable documentation

Both `group_consumer` and `group_reader` were built and executed against the same
fresh root under `target/group-smoke-fb539e05-5d2f-438c-a438-189613c0556b`.
The CLI registered a local source and acquired its output; the native example
leased its directory, and `run-group` passed the live descriptor to the child
example, which listed the payload. The standard default user-home store was
separately tested with an isolated home and no app or credentials.

## Final checks

| Check | Result |
| --- | --- |
| `cargo test --locked --all-targets` | 283 passed, zero failed, five optional native-keyring tests ignored across 50 targets |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `openspec validate --all --strict` | 22 items passed, zero failed |
| Affected documentation links | 23 local targets exist |

The final full run includes the create-new descriptor write and isolated marker
fixture. Windows console interruption ran successfully in the focused group
suite. All 16 tasks are complete. On 2026-10-14 the change was synced to the
main specifications and archived as `2026-10-14-add-group-leases`; all 22 main
specifications passed strict validation. Temporary change-state cleanup succeeded
with no state to remove.

The full suite log is `target/group-leases-full-tests.log`. Counts use each Cargo
target's final summary, excluding nested child-fixture summaries.

No hosted workflows, Linux/macOS runs, native keyring smoke tests, MSRV runs,
external consumer edits or push were part of the runtime verification. Optional
native keyring tests remain opt-in; injected-key and basic-mode fixtures cover the
app and integrity contracts without assuming a usable OS keyring. Persistent
group registration, retrieval orchestration and atomic multi-resource updates
remain separate future work. Finalized specifications and archived artifacts are
committed; active change artifacts stay outside commits.

## Dependency decisions

Existing serde/serde_json, UUID validation and the flat catalog cover the bounded
request and policy contract. Existing file locks, lifetime markers, temporary
workspaces and RAII cover retention and rollback. The interactive launch helper
preserves inherited streams and exec/spawn behavior; process-wrap remains for
existing bounded capture jobs. No graph engine, IAM engine, asynchronous runtime,
Python binding or additional lock crate is needed for this finite group feature.
