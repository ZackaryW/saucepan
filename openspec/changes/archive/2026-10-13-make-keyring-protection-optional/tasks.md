# Tasks

## 1. Mode-aware integrity foundation checkpoint

- [x] 1.1 Add the integrity mode model and basic checksum codec alongside the exact current protected codec, reusing existing Rust digest/HMAC primitives; verify known vectors, bounded parsing, unknown/duplicate fields, identity/generation/payload changes, mode separation, and rejection of checksum-for-HMAC substitution with focused unit tests.
- [x] 1.2 Integrate explicit mode selection and mode-preserving initialization reservations into metadata persistence while retaining existing production defaults until checkpoint 3; verify basic create/open through a provider that rejects every custody call, interrupted retry identity/mode matching, and protected missing-key failure without replacement or conversion.
- [x] 1.3 Parameterize catalog, transaction, membership and publication-recovery tests over basic and injected protected integrity; verify corrupt/missing members, mixed-mode journals, old/new publication boundaries, and idempotent replay without network or client credentials.
- [x] 1.4 Document the new native integrity contract and its filesystem trust boundary in `docs/claim-integrity.md`, clearly identifying API availability versus the still-unchanged CLI default; verify the documentation against the focused tests, run formatting and relevant Clippy checks, inspect the diff and commit this coherent foundation before dependent work.

## 2. Independent credential proof checkpoint

- [x] 2.1 Add an explicit independent bearer-verifier variant and domain/store/app/generation binding using existing constant-time HMAC primitives; verify correct, missing, wrong, cross-app, cross-store and old-generation tokens, including that verifier bytes cannot themselves serve as a bearer token.
- [x] 2.2 Preserve the existing protected verifier variant and use the independent variant for new credential issuance in both modes; verify existing protected tokens without reissue, explicit rotation, generation/revision conflicts, retry non-disclosure and rejection of signing-key-dependent variants in basic stores without keyring access.
- [x] 2.3 Exercise registration/session policy behavior in both modes: credentials disabled/enabled, forbidden source/action/scope, public discovery and native lease survival after rotation; verify tokens remain absent from serialized metadata, ordinary results and diagnostics.
- [x] 2.4 Update credential documentation with the independent proof contract and basic-mode filesystem limitation; run the focused credential/session checks, formatting and relevant Clippy, then inspect and commit this verified slice before changing defaults.

## 3. Default basic operation and real-process fixtures checkpoint

- [x] 3.1 Make simple native creation and CLI `init` default to basic mode, add explicit `--integrity basic|keyring` and native creation options, expose selected mode and dispatch opening from persisted format; verify help/default-home/explicit-root behavior, invalid mode before effects, and protected current-format opening without a metadata rewrite.
- [x] 3.2 Convert ordinary native-root/CLI fixtures to basic mode, preserve explicit injected protected fixtures, and move actual native service tests behind explicit selection in the same slice; run `cargo test --locked --all-targets -- --nocapture` without native keyring setup and verify ordinary paths make no native custody calls, including teardown.
- [x] 3.3 Verify source/claim queries, access levels, recovery, activation/reclaim and manager/shim use the shared integrity context; add focused real-process tests for basic init-to-query, basic app credentials, default launch, protected unavailable-key rejection and unchanged source/scope visibility. Preserve lifecycle race assertions and probe cleanup/deadlines.
- [x] 3.4 Update README, init help, relevant examples and integrity docs to lead with basic initialization and explicit protected opt-in; run documented basic commands in a fresh disposable root, formatting and relevant Clippy, inspect the integrated diff and commit before workflow changes.

## 4. Explicit native verification and CI checkpoint

- [x] 4.1 Complete the selected native suite with round-trip/cleanup, concurrent stores and actual test-executable/manager/shim interoperability; add bounded subprocess waits and stage-specific secret-free error reporting. Verify the failure path deterministically and run native checks only on an available configured local host, recording other platforms as unverified.
- [x] 4.2 Add default-false `check_keyring` inputs to manual/reusable platform checks and manual release forwarding; retain the ordinary full release matrix, manual platform/MSRV controls, exact checked SHA, streaming logs and build gating. Verify workflow expressions for manual basic-only, selected native, tag release and manual release paths with actionlint and focused workflow-selection checks; do not dispatch hosted runs.
- [x] 4.3 Make Linux credential-service setup conditional on native selection while preserving ordinary Git LFS prerequisites; document exact normal/native commands and not-run versus passed reporting in `docs/releases.md`. Verify native selection actually executes the intended tests and an explicit native failure cannot become success or an automatic skip.
- [x] 4.4 Run focused workflow/package checks and inspect the native test/docs/workflow diff, then commit this verified slice; record the macOS native failure as unresolved unless a real native test demonstrates otherwise.

## 5. Integrated acceptance and reconciliation checkpoint

- [x] 5.1 Run the existing real-build release-fixture acceptance with a fresh basic root and actual built manager/shim through installation, successful/failed update, rollback, deactivation/reactivation, shared-output uninstall and reconciliation; record build identities and outcomes and verify historical `mock/` remains unchanged.
- [x] 5.2 Run final formatting, locked Clippy, relevant full Rust tests and packaging checks after integration; report exactly which native platforms and MSRV checks executed, with no inference from basic success to native custody support.
- [x] 5.3 Reconcile implementation documentation and the change deltas against the final behavior, check current spec cross-references for unconditional HMAC language, validate this change strictly and inspect the final diff; commit any coherent remaining acceptance/documentation changes. Leave baseline spec sync and archive to their separately requested workflow.
