# Tasks

Each group is a coherent implementation checkpoint with its own tests and documentation. Review the diff and commit explicit implementation paths after relevant checks pass, following zmem authoring guidance and existing authorization. Active OpenSpec change files stay outside those commits. Planning does not authorize implementation, external consumer edits, store migration, pushing or hosted CI.

## 1. Group contract and restrictive policy evaluation

- [x] 1.1 Add strict request/member/descriptor types under models/lease with named output/claim targets and directory views; verify JSON round trips, omitted policy defaults, name/member/reference/input limits, duplicate names, unknown fields, malformed UUIDs and path rejection in focused tests.
- [x] 1.2 Add group-only boundary evaluation under core/policy using existing leaf definitions/catalogs; verify intersected source/action/scope allowances, omitted versus empty dimensions, output targets without scope witnesses, wrong-target policies, duplicate singleton conflicts and caller-catalog shadowing.
- [x] 1.3 Integrate per-member existing authority and path-verification evaluation without relaxing ordinary request grants; verify denied app requests remain denied, trusted local groups obey voluntary boundaries, group metadata cannot weaken full verification, and aliases cannot mask denied members.
- [x] 1.4 Add the request/policy section of docs/group-leases.md with unrestricted and app-scoped examples; verify the documented JSON with contract tests and run focused policy tests plus formatting before checkpointing.

## 2. Coordinated native group ownership

- [x] 2.1 Add core/lease admission and minimal locked-access helper refactors: select/authorize members from coordinated metadata, validate views and acquire deduplicated output guards in UUID order under one remaining-time budget; verify empty/invalid groups have no effects, two names preserve distinct views, and reversed-order groups plus reclaim do not deadlock.
- [x] 2.2 Perform strongest-required scans outside global coordination and recheck relevant app/source/claim evidence before success; verify concurrent update, release, uninstall, credential rotation and policy adoption fail admission cleanly while unrelated policy registration and unrelated reads remain usable.
- [x] 2.3 Expose Store and AppSession group APIs with one non-serializable guard; verify complete rollback after later-member failure, metadata-only access without payload reads, independent groups sharing outputs, no native process markers and existing groups retaining old outputs after subsequent claim/policy changes.
- [x] 2.4 Document and compile a native group-consumer example with optional app identity, live named views and explicit drop; verify no source fetch, recovery, synthetic claim, group registration, policy adoption or revision-history change occurs, and run focused access/reclaim/scoped-consumer regressions before checkpointing.

## 3. Multi-resource child lifetime and CLI

- [x] 3.1 Extend the dedicated launch boundary to retain multiple existing lease markers and temporary descriptor ownership, using current lifetime and lock utilities; verify two-marker inheritance, partial setup failure, failed spawn, normal exit and native non-inheritance with real isolated process fixtures before CLI integration.
- [x] 3.2 Implement bounded run-group request/catalog parsing and optional app/credential dispatch using the native group API; verify default and explicit roots, output-only credential-free operation, protected app failures, no administrative fallback and structured admission errors without partial stdout.
- [x] 3.3 Publish the owned UTF-8 descriptor and SAUCEPAN_RESOURCE_GROUP_FILE child environment; verify label/path mapping, duplicate-output views, Unicode/space-containing paths, no secrets, literal arguments, unchanged working directory, inherited streams, exit/interruption handling and removal of stale singular child variables without changing the parent.
- [x] 3.4 Verify wrapper termination preserves every output and the descriptor until supported child ownership ends, and cleanup removes only closed owned markers/scratch; exercise reclaim while all three resources are used and regression-test single run/run-claim, temporary consumers and managed shims on each executed host.
- [x] 3.5 Extend docs/group-leases.md, scoped-consumer guidance and CLI help with a runnable child example, preparation/recovery separation and process-inheritance limits; execute the examples on disposable roots and run relevant CLI/lifetime checks before checkpointing.

## 4. Combined acceptance and final reconciliation

- [x] 4.1 Add a Zush-shaped acceptance fixture using separate command, hook and service outputs, covering both unrestricted and app-bound groups, a narrowing policy, duplicated views, updates/releases during use and reclaim after group exit; prove denied preparation starts no consumer without editing Zush or Power Eagle.
- [x] 4.2 Run cargo fmt --all -- --check, cargo clippy --locked --all-targets -- -D warnings and cargo test --locked --all-targets after integration; report exact executed platform/results and optional unrun custody/platform checks without triggering hosted CI.
- [x] 4.3 Reconcile proposal/design/deltas, README and group-consumer evidence with the implemented contract; run strict OpenSpec validation, review the final diff and record verified zmem checkpoints while retaining active change files outside implementation commits. Confirm persistent groups, automatic retrieval and coordinated updates remain deferred and no compatibility adapters were introduced.

## Completion evidence

Windows x86_64, Rust 1.98.1: all-target suite 283 passed / 0 failed / 5 optional keyring tests ignored across 50 targets; formatting and strict Clippy passed. Strict OpenSpec validation passed 22 items. See `docs/group-lease-verification.md` for scenarios, executed examples, limits and dependency decisions. Checkpoints `565cf23`, `24e1115` and `fd23879` delivered contracts, native ownership and CLI lifetime respectively; final verification is recorded in a separate explicit-path zmem checkpoint. Active change files remain uncommitted. No external consumer edits, hosted CI, compatibility adapter, sync or archive were performed.
