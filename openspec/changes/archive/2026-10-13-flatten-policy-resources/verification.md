# Flat-policy verification — 2026-10-13

This records the `flatten-policy-resources` implementation on Windows x86_64
(`x86_64-pc-windows-msvc`, Rust 1.98.1). All inputs use the reference-only contract.
The source, retrieval, acquisition, claim and path APIs resolve typed leaf records;
there is no inline policy adapter or previous-layout decoder.

## Integrated consumer flow

`tests/policy_acceptance.rs` runs a Power Eagle-shaped consumer against a local Git
repository and a fresh basic-mode root. It registers immutable definitions and a
credential-free app restricted to two explicit scopes, then verifies:

- Identical retries share a claim within one scope; separate scopes keep separate
  claims and can share output bytes.
- Registering a new update-policy UUID changes nothing until one claim adopts it
  at the observed application revision. The other scope and historical output
  metadata stay unchanged.
- A changed Git commit outside the selected subpath reuses the output; changed
  selected files create a new output and advance only the updating scope.
- Latest-two history and reclaim respect a live native lease. The consumer copies
  while holding that guard; its owned copy survives reclaim. Exact recovery
  restores the other claim's original Git contents.
- Releasing both claims leaves no ownership records and retains immutable policy
  definitions.
- A child with HOME/USERPROFILE removed retrieves the same Git source through a
  portable no-store policy and copies its subpath to caller-owned output. It
  creates no Saucepan root, policy catalog, index or claim, and does not change
  the managed root.

No Power Eagle files were edited. This validates the native consumer contract,
not Power Eagle's existing adapter or a cross-language IPC lease bridge.

## Contract and replay checks

`policy_composition` verifies complete two-stage replay evidence, equivalent
policy-ID deduplication, scoped caller-receipt reopening, release filtering and
registered fresh operations without writes. `policy_cli` exercises exact lookup,
revision-checked adoption, flag/reference conflicts and rejection of the removed
global refresh/stale flags before initialization.

`metadata_integrity` rejects earlier roots before custody or mutation in both
basic and injected-key modes. It also injects authenticated mixed records to
verify strict typed rejection without rewriting bytes. Missing lifecycle fields
and missing/null adopted output evidence are rejected. Optional public input
fields still have documented defaults; those defaults are not old-schema readers.

The retained 2026-10-10 release binary also refused a newly initialized flat root:
exit 1, no stdout, `metadata integrity verification failed`, and every root file
unchanged. Its SHA-256 was
`70ed784597e9aae786e4874a444bc412035a15bd80ba48cba428ea73815a0fbb`.

## Documentation and validation

Fourteen real CLI operations executed the README acquisition example, claim CLI
registration/ensure/recovery example, and policy guide registration/UUID lookup
against separate disposable roots. Only illustrative paths and returned UUIDs
were substituted. Composition JSON examples are checked by Rust contract tests;
consumer examples compile as all-target Cargo checks.

| Check | Result |
| --- | --- |
| `cargo test --locked --all-targets` | 264 passed, zero failed, five optional native-keyring tests ignored across 47 targets |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `openspec validate --all --strict` | 20 items passed, zero failed |
| Local documentation links | 65 targets exist |

Counts use each target's final summary, excluding duplicate child-fixture summaries.
Earlier runs exposed incomplete claim/retrieval test builders after strict
evidence validation; those builders were corrected before the successful full
run. The final suite log is `%TEMP%/saucepan-flat-complete-suite.log`.

All 29 implementation tasks in `flatten-policy-resources` were completed. The
composition and flat-policy changes were subsequently synchronized and archived
on 2026-10-13, in that dependency order.

At that verification checkpoint, no hosted workflows, native OS keyring smoke
tests, Linux/macOS execution, MSRV execution, historical mock migration, push,
sync or archive were performed.
Injected custody verifies protected behavior without asserting OS keyring setup.
Crate inclusion and deferral reasons remain in
[composition utilities](utilities.md#crate-reuse-decisions).

This report retains the flat-policy checkpoint's dated evidence. See
[group-lease verification](../2026-10-14-add-group-leases/verification.md) for the subsequent grouped
consumer extension. Historical verification reports retain their dated evidence
and are explicitly marked as earlier contracts.
