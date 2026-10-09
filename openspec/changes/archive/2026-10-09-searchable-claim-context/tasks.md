# Tasks

Each numbered group is a coherent checkpoint. Complete its implementation, relevant tests, and documentation before starting dependent work. When committing is authorized, inspect the diff and commit only explicit implementation paths; exclude active OpenSpec change artifacts and `openspec/.pspec/`. Keep all boxes pending until implementation evidence exists.

## 1. Portable string-pattern utility

- [x] 1.1 Implement `utils::pattern::StringGlob` with the specified syntax, compile-time errors, Unicode-scalar matching, and iterative bounded-space evaluation; verify domain-neutral unit cases for anchoring, empty values, escaping, separators, Unicode, malformed syntax, repeated stars, and long nonmatches.
- [x] 1.2 Document the helper's pure string contract in Rust API docs and compare bounded generated pattern/value cases against an independent reference matcher; run the focused utility tests and formatting checks before checkpointing this slice.

## 2. Persisted context and safe claim transitions

- [x] 2.1 Add `ClaimContext`, validation, and optional empty context to claim serialization; implement `acquire_for_with_context` while retaining empty-context acquisition and updating all claim constructors. Verify round trips, omitted/invalid/over-limit inputs, failure before acquisition, distinct project claims sharing one output, and equality of returned and persisted context in isolated claim tests.
- [x] 2.2 Implement revision-checked whole-context replacement through the existing index journal. Verify replace, clear, no-op, stale revision (including an otherwise equal map), missing/removed/removing app or claim, unchanged selections/binding, and recovery before/after record publication.
- [x] 2.3 Preserve commit-time context during primitive updates and actual committed context in their results; retain context through managed updates, rollback, deactivation, and recovery. Verify controlled concurrent context replacement during primitive acquisition, managed revision conflicts, successful managed transitions, release/uninstall cleanup, empty managed installation context, and survival after output reclaim.
- [x] 2.4 Document native context APIs and concurrency semantics in `docs/claim-query.md`; run affected claim, index-recovery, and lifecycle tests plus `cargo check --all-targets` so updated model constructors remain coherent before checkpointing.

## 3. Application-scoped query service

- [x] 3.1 Add `core/query/{mod,model,matching}.rs` and export its public types. Verify strict JSON shape, required app/where, supported operators, input limits, nonempty keys, eager validation on zero candidates, AND semantics, missing keys, literal dotted keys, and explicit equality/glob behavior with focused unit tests.
- [x] 3.2 Implement `Store::query_claims` using one coordinated metadata snapshot and existing source filters, without nested lock acquisition. Add isolated native integration tests proving two-project selection within one app, other-app exclusion, duplicate-context results, UUID ordering, empty results, invalid app state, matched output corruption errors, and reclaimed availability.
- [x] 3.3 Verify query/update snapshot consistency with controlled concurrent mutations, no source refresh or payload traversal, unchanged application revisions, and absence of retained reclaim protection. Extend `docs/claim-query.md` with query/result examples and exact-output versus current-claim lease guidance; run focused query and resource-access regression tests before checkpointing.

## 4. CLI integration and runnable examples

- [x] 4.1 Add `acquire --context`, `set-claim-context ... --expected-revision`, and `query-claims FILE` adapters. Verify subprocess JSON results, native/CLI parity, explicit-root behavior, context-without-app rejection before acquisition, invalid input/error exits, and successful zero/multiple-match queries.
- [x] 4.2 Add example context/query files and README commands linked to `docs/claim-query.md`. Exercise the documented flow against a temporary store: acquire shared content for two project contexts, search with exact/glob conditions, replace using an observed revision, and inspect the independent claims; verify no changes to the existing `mock/` installation.
- [x] 4.3 Run whole-change integration checks (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `openspec validate searchable-claim-context --strict`). Inspect the final diff and record actual platform evidence and any unexecuted platform checks before the final implementation checkpoint.
