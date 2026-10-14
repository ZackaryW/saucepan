# Resource group and pin verification — 2026-10-14

This records `orchestrate-resource-groups` on Windows x86_64
(`x86_64-pc-windows-msvc`, Rust 1.98.1, Cargo 1.98.1). The implementation uses
`models/{group,pin}`, `core/{group,pin}` and CLI adapters, with the existing
verified metadata journal, native guards and policy resolver. No crate or
compatibility adapter was added.

## Consumer fixtures

`tests/resource_group_cli.rs` creates fresh basic-mode roots and real registered
UUIDs. These are consumer-shaped fixtures, not executions of the external apps.

- **Power Eagle:** an updater process prepares and pins A, returns its exact paths,
  and exits. A separate reader owns no Saucepan native guards. Another updater
  prepares/pins B and moves the selection while reclaim still protects A. The
  delayed reader reads A's original bytes; explicit unpin then permits reclaim.
- **Zush:** local and app-owned command/hooks/service members launch under one
  retained generation. Updating and removing the registration does not redirect
  or delete the running child's files. After the child exits, obsolete outputs
  become reclaimable. Launch forwards literal arguments and exit status; spawn
  failures and failed preparations emit no partial success JSON.
- Pin receipt retries return the original operation result. Expected-revision
  conflicts, malformed inputs and named member failures are exercised through the
  CLI. JSON examples in the pin/group guides are parsed by tests.

`tests/output_pins.rs` additionally checks independent consumer pins, process exit
and reopen, history-none retention, claim movement/release, explicit cleanup,
current source/scope/action permissions, full-verification rollback, concurrent
registration, corrupt metadata refusal and orphan cleanup after app uninstall.
Inspection distinguishes the recorded output state from live metadata availability;
missing payloads do not prevent authorized receipt inspection or release.

`tests/resource_groups.rs` checks pure/idempotent registration, local/app separation,
strict bounds and policy roles, manual reuse, distinct local update policies,
covered claims and binary views, explicit scope independence, common Git commit
selection, permitted stale fallback, atomic failure, old pin receipt retries and
current authorization for every member sharing one output.

## Concurrency and publication

Internal group fixtures inject changes to claims, context, adopted policies,
source policy, credentials and registration during preparation. Final fences
reject stale publication without resurrecting removed state. Recovery tests cover
report-unavailable, restore-exact and refresh. Exhausted ordered-lock budgets
release earlier guards.

The group journal fixture interrupts each observed final publication boundary
and reopens the store: the generation, claims, successful-check records and
explicit pin recover together. A retry returns the original pinned receipt.
Existing journal tests cover shared publication in both basic and injected-key
integrity modes. The group fault fixture uses injected custody; normal CLI
fixtures use basic integrity. These do not test OS credential-manager availability.

Atomicity covers the final metadata selection. Failed preparation can leave
complete unselected cache entries, which remain reclaimable. It does not roll back
provider downloads, plugin activation, executed hooks or caller-owned files.

## Checks

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `cargo test --locked --all-targets` | 319 passed, 0 failed, 5 ignored across 57 target summaries |
| `openspec validate --all --strict` | 23 passed, 0 failed; informational long-requirement notices only |
| `cargo test --locked --test output_pins --test resource_group_cli --test resource_groups` | Final inspection follow-up: 27 passed |
| `cargo test --locked --lib core::group::tests` | Final inspection follow-up: 4 passed |

The full suite began at the CLI checkpoint; the final pin-inspection adjustment
also received the focused follow-up checks above and a fresh formatting/Clippy
pass. The full run includes the final 9-test output-pin fixture. Test totals
include process-helper test entries; follow-up totals are reruns, not additional
unique tests. These logs are local build artifacts under `target/group-final-*`
and `target/group-inspection-*`.

The five ignored tests explicitly require configured native credential custody:
one protected CLI flow, a native keyring round trip, independent store-key
creation/retirement, independent-process initialization and manager/shim custody.
They were not selected for this run. Basic and injected-key behavior passed.

Checkpoints: `27dee74` (pins), `15db709` (registration), `13064db` (preparation and
publication), `ba8e83a` (CLI/consumer integration), `8d99f82` (live pin inspection).
Each commit message passed zmem validation and its resulting entries were checked.
Active OpenSpec artifacts remain excluded from commits pending archive.

No hosted workflow was triggered. Historical `mock/` and the Power Eagle/Zush
repositories were not modified. Actual ESM plugin imports/activation, Zush Python
and service shutdown, detached descendants, Linux/macOS behavior, the minimum
supported Rust version and native credential-manager availability require their
own integration/environment validation. Consumers must preserve inherited handles
or explicitly own durable pins for the full period of use; a serialized path is
not a lifetime guard or permission token.
