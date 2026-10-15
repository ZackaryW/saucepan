# Tasks

Each numbered group is a coherent implementation checkpoint including its own tests and user-facing guidance. After relevant checks pass, inspect and stage only that checkpoint's implementation/docs/tests and create a local zmem-authored commit before dependent work. Exclude active OpenSpec changes and unrelated files; do not push. Reuse existing utilities and crates as recorded in design.md. All boxes below remain pending until implementation evidence exists.

## 1. Pure profile expansion and explicit registration workflow

- [x] 1.1 Add typed profile inputs/results under `src/models/policy` and Rust-owned recipe expansion under `src/core/policy`; verify fixed leaf UUID/body fixtures, recipe digests, shared leaf identities, three recipe matrices and managed as the sole expansion default with focused profile tests.
- [x] 1.2 Add bounded stage-specific override composition and for-install expansion using existing catalog/axis validation; verify wrong-target, unknown, duplicate and competing policies, custom retention, temporary-install rejection and no mutation of immutable definitions.
- [x] 1.3 Wire `profiles` and `profile` before root/home lookup in the CLI, retaining explicit ordinary `register-policies`; verify root-free preview, repeated idempotent registration, no automatic attachment, app-bound registration denial, no-profile regression behavior and portable catalog use through CLI tests.
- [x] 1.4 Add README-linked profile usage instructions with the exact matrix, ordinary leaf payloads, separate retrieve/acquire refs, explicit adoption and protected-history wording; execute the documented managed and root-free temporary flows against isolated fixtures before checkpointing this slice.

## 2. Effective-policy explanations and composition boundaries

- [x] 2.1 Add typed explanation inputs/results and shared evaluator tracing for retrieve/acquire/compose/claim; verify origin level, UUID/digest versus built-in evidence, applicable-axis reporting and adopted-claim behavior after app-default changes in policy tests.
- [x] 2.2 Add `explain-policies` with one verified snapshot and normal read/session authorization; verify no provider/probe/write effects, no-root inline explanations, scoped denial without payload leakage, unknown references, missing/corrupt catalog records and consistent errors against actual planning.
- [x] 2.3 Exercise expanded profiles through existing composition, claim/adoption and reclaim paths; verify 24-hour retrieval reuse versus explicit-update origin checks, unchanged-content reuse, manual acquisition updates, registered constraint intersection, revision-checked adoption and pins surviving outside the latest-ten window.
- [x] 2.4 Document explanation commands and the distinction between preview evidence and execution authority in the existing user guide; run the examples plus affected policy CLI/composition tests before creating the local checkpoint.

## 3. Managed application and Saucepan setup integration

- [x] 3.1 Connect for-install expanded refs to existing managed install/bootstrap paths and enforce any missing preflight checks without replacing lifecycle machinery; verify cached storage, required usable-app protection, explicit scope/platform/probe inputs and temporary/no-store rejection before provider work.
- [x] 3.2 Verify installation/update/rollback with profile-expanded refs through existing lifecycle fixtures, including failed probes, concurrent reclaim, interrupted commit, original-operation retry and changed-input conflict; preserve prior claim/binding and truthful retained registration/cache side effects.
- [x] 3.3 Update `docs/application-lifecycle.md` and README setup guidance with first manager/shim extraction, basic initialization, profile/source/filter registration, bootstrap, self-update and PATH instructions; execute the basic flow using a controlled release fixture and actual built binaries, leaving the historical mock and real home untouched, before checkpointing.

## 4. Typed SDK profile and managed lifecycle operations

- [x] 4.1 Extend `sdk/contracts/operations.json`, both model sets and client mappings for profile listing/expansion and explanation; verify cross-language parity, object inputs, optional-field omission, Rust-owned expansion, root-free preview and identity preservation with fake transports and real-manager fixtures. Add runnable SDK profile examples with this slice.
- [x] 4.2 Add typed lifecycle response handling for complete and exit-2 receipts/lists while preserving existing operation-specific errors; verify pending, failed, conflict, read-only inspection, mixed reconciliation results, malformed/status-inconsistent output and indeterminate mutation behavior in both transport suites.
- [x] 4.3 Add install/bootstrap/update-app/self-update/rollback/uninstall/operations/reconcile wrappers with explicit request objects, built shim, scope and operation IDs; verify literal argv, application/admin boundaries, same-receipt retries initiated only by callers and missing executable errors without auto-provisioning.
- [x] 4.4 Update both SDK READMEs, shared SDK guide and examples to distinguish resource consumers, managed installation and excluded standalone shim/provisioning operations; verify temporary guarded lifetime/caller-owned delivery and no implicit credentials, scope, pin or SDK-import effects. Run both SDK unit/type/build checks and affected real-manager contract fixtures before checkpointing.

## 5. Cross-surface acceptance and release readiness evidence

- [x] 5.1 Extend isolated actual-build acceptance across profile expansion, policy registration, bootstrap, shim launch, explicit update to distinct binary bytes, failed replacement and reclaim; verify receipts, exact identities/digests and old-selection protection. Record actual platform evidence and keep native keyring checks separately selected.
- [x] 5.2 Build/install Python and TypeScript artifacts outside the checkout and run profile/explanation plus lifecycle flows using explicit built manager/shim paths; verify no private imports, checkout dependencies, duplicated recipe tables or false temporary handles and inspect recorded pending/completed results.
- [x] 5.3 Run appropriate full Rust formatting/clippy/test checks after focused slices pass, complete SDK contract/package checks, and strict OpenSpec validation; reconcile README/docs/contracts against delivered behavior and report unavailable platforms honestly. Review the final diff and create the final local implementation checkpoint without active change files, hosted workflow dispatch or pushes.
