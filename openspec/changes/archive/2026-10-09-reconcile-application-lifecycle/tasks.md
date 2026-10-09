# Tasks

One coordinated change with five dependency-ordered checkpoints. Each checkpoint includes its own tests and documentation; commit its verified implementation before dependent work, excluding `openspec/changes` and unrelated files. The final integrated acceptance is required even when individual checkpoints pass.

## 1. Recoverable lifecycle state and operation contracts

- [x] 1.1 Add app readiness/removing state, mutation revision, shim ownership, operation intent/result models and narrow operation phases; update every existing app mutation to advance revisions only on actual writes and reject removing state; verify strict decoding, default loading of current records, idempotent deactivation and existing claim tests.
- [x] 1.2 Extend the short journal for operation/app metadata transitions and safe app-directory retirement; serialize same-operation execution and implement input fingerprint/idempotency rules; verify replay before/after every record write/retirement and ensure index scanning never encounters an intentionally empty UUID directory.
- [x] 1.3 Add native operation inspection/reconciliation APIs and CLI `operations`/`reconcile` with truthful complete/pending/conflict statuses and exit codes; verify repeated identities, changed-input rejection, unknown IDs and that pending filesystem work does not block unrelated app operations or trigger downloads/probes during store open.
- [x] 1.4 Document the lifecycle operation/recovery contract and lock order with executable CLI examples; run focused model/journal/CLI checks and review/checkpoint the slice before proceeding.

## 2. Owned launcher installation and maintenance

- [x] 2.1 Implement executable/sidecar ownership and reservations in `core/shims`, including case/sidecar alias collisions, links, occupied unowned paths and concurrent operations; verify both-file preflight and no overwrite in focused filesystem tests.
- [x] 2.2 Route `install-shim` through recoverable publication, add explicit adoption and replacement, and coordinate launcher reads/Windows lifetime with maintenance locks; verify old-pair preservation, missing-file restoration, changed-file conflicts, same-operation retries and crash recovery at each publication boundary using built shims.
- [x] 2.3 Expose maintenance APIs/commands and document ownership, adoption, pending cleanup and the old-writer upgrade boundary; verify native/CLI parity and copied existing-sidecar adoption in temporary roots, then checkpoint the tested slice.

## 3. Coordinated installation, update and rollback

- [x] 3.1 Refactor candidate preparation to return a protected output without publishing an app claim; add durable commit-intent candidate protection to reclaim and integrity-checked recovery; verify lease handoff without a reclaim gap, process exit after intent, corrupt/missing candidates and release of temporary protection on terminal outcomes.
- [x] 3.2 Implement generic install using reserved app/claim/shim identities and journaled final publication; route bootstrap through the same coordinator; verify failed validation leaves no live partial app, interruption resumes the same identities and conflicting shim reservations cannot create duplicate usable installs.
- [x] 3.3 Implement managed update/rollback with one claim/binding commit and revision conflict checks; route self-update through it while preserving primitive claim/activate/deactivate semantics; verify failed probes preserve both selections, rollback item checks and original requests, ABA/intervening mutations, settings preservation and native/CLI parity.
- [x] 3.4 Document install/update/rollback and the explicit distinction between managed-operation conflicts and binding-only last-commit activation; run focused lifecycle/bootstrap/managed/reclaim tests and checkpoint the integrated slice.

## 4. Uninstall with deferred owned cleanup

- [x] 4.1 Implement logical removal and fence all in-flight claim, activation and maintenance publication; verify no resurrection using deterministic barriers against managed and binding-only probes, claim updates and shim operations while existing leases remain usable.
- [x] 4.2 Implement identity-checked shim cleanup, unregistered-shim adoption/exclusion handling and safe final app retirement; verify busy/missing/changed files, unknown directory contents, interrupted retirement, repeated removal and unrelated unknown UUID errors without recursive deletion of unknown content.
- [x] 4.3 Expose uninstall with typed pending results and direct-binary reconciliation; document self-uninstall and normal retention after claim removal; verify real Windows launcher contention, shared outputs/other active apps, live native/process consumers and surviving output-policy demand, then checkpoint the tested slice.

## 5. Whole-lifecycle reconciliation and acceptance

- [x] 5.1 Add and run an isolated real-build lifecycle recipe covering install A, update B, failed update, rollback, deactivate/reactivate, shim replacement, two apps sharing resources, live-target uninstall, pending cleanup, reconcile and repeated removal; record digests/operation IDs/claims/bindings/files/retention and prove the existing mock digests unchanged.
- [x] 5.2 Run the combined deterministic failure/concurrency matrix and relevant existing access/marker/managed-console regressions, then `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and strict OpenSpec validation; record actual host coverage and any unavailable checks without claiming runtime parity elsewhere.
- [x] 5.3 Reconcile README, lifecycle examples and verification evidence against the design's operation/earlier-contract matrices; verify every spec scenario maps to a passing check, review the final diff and checkpoint integration evidence. Leave completed earlier changes unarchived and planning artifacts excluded from implementation commits.
