# Tasks

Each numbered group is a dependency-ordered implementation checkpoint, including its tests and documentation. Scope and defaults are defined in proposal.md, design.md, and the two delta specs. Report each verified checkpoint and a proposed commit; create commits only if separately authorized. All tests use explicit isolated roots; preserve existing mock contents and do not replace its stable shim implicitly.

## 1. Shared resource lifetime and reclaim coordination

- [x] 1.1 Extract the existing native lease composition into core/store access ownership and introduce a non-serializable ResourceLease carrying the exact resolved entry and directory, initially retaining full verification; preserve the existing execution_lease API contract and marker compatibility, and verify current managed/reclaim tests still pass without a second lease implementation.
- [x] 1.2 Add coordinated entry and claim lease acquisition with mutation-then-entry lock ordering, shared consumer ownership, and native lifetime cleanup; verify real filesystem/process fixtures cover multiple holders, final release, claim update/release, unknown claims, reclaim winning selection, and inherited markers left by an existing launcher.
- [x] 1.3 Document guard ownership and the distinction from ordinary claims and copied paths; execute a native usage example that reads a leased file while reclaim defers it, then drops the guard and observes normal reclaim without changing request/provenance/creation order.

## 2. Explicit lightweight resolution and full verification

- [x] 2.1 Add VerificationMode and ResolvedResource, metadata resolution, full verification under a temporary or retained entry lease, and the metadata-default Store::path adapter; verify UUID/type/state/root/containment failures, exact-old-entry lookup, unchanged identity/freshness, and clear verification-level results.
- [x] 2.2 Reuse the bounded manifest inspector for full access and add a narrow per-test IO observer/barrier; verify metadata lookup performs zero payload traversal/reads, full access scans once, same-length tampering and missing/extra/type-changed content fail, and a paused full scan protects its entry while unrelated lookup/index mutation completes.
- [x] 2.3 Add CLI path --verification and verify commands with matching JSON/error semantics; execute examples and tampering fixtures through both native and CLI access, and document the default behavior change, point-in-time integrity limits, and lack of lifetime protection after bare lookup returns.

## 3. Managed launch and mandatory integrity boundaries

- [x] 3.1 Route shim binding selection and executable validation through one metadata resource lease; verify target membership, unsafe/missing/non-file executables, explicit-root propagation, active-binding selection, and zero wrapper payload hashing using the rebuilt shim or an observed equivalent launch boundary.
- [x] 3.2 Audit every full-check caller in acquisition, remote cached-copy reuse, journal recovery, activation, and rollback; remove only same-phase duplicate checking, perform activation's pre/post-probe scans under its lease outside the global lock, and verify corrupted reuse, probe-mutated candidates, failed activation, and interrupted publication preserve the existing integrity guarantees.
- [x] 3.3 Update managed-app documentation for lightweight routine launch and unchanged activation validation; run the real managed-child regressions for running-old-target activation/rollback, wrapper death, streams/status, and available platform interruption, explicitly recording unavailable OS execution rather than claiming parity from conditional compilation.

## 4. Leased CLI execution for general consumers

- [x] 4.1 Add run ENTRY_UUID --verification MODE -- PROGRAM ARG... using the shared private process boundary and SAUCEPAN_RESOURCE_DIR; verify real child fixtures receive the exact directory, overridden child-only environment value, literal arguments, inherited working directory/streams, and original exit status without wrapper JSON on stdout.
- [x] 4.2 Confine handle inheritance to dedicated launch processes and preserve Unix exec/Windows wait semantics; verify spawn/exec failure, failed full verification, ordinary guard non-inheritance, consumer exit, wrapper termination, concurrent reclaim, and supported console/signal interruption without introducing PID-only or timeout-based ownership.
- [x] 4.3 Document a CLI data-consumer example alongside native lease access, including claim-independent lifetime and targets that discard inherited handles; execute it while retention exceeds its window and the associated claim is released, confirming deferred content survives use and becomes reclaimable afterward.

## 5. Integrated acceptance and review

- [x] 5.1 Extend acceptance recipes outside mock to exercise lightweight lookup, explicit corruption detection, claim-based native access, leased external consumption, and reclaim in isolated roots; run a freshly built application/shim installation, record binary identities and observed payload-read counts, and preserve any existing mock installation without implicitly replacing its shim.
- [x] 5.2 Run formatting, strict Clippy, all-target build, the relevant full suite, available native process/console checks, and strict OpenSpec validation; map each new scenario to actual evidence, report unavailable platform/toolchain checks, and review the diff for unintended schema, provider, retention, SDK, or production-root changes before declaring completion.
