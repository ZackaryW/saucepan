# Tasks

## 1. Native source discovery checkpoint

- [x] 1.1 Add authenticated private-default visibility and trusted configuration; verify publish/withdraw, registration retry, cache preservation and app-session rejection in core tests.
- [x] 1.2 Add strict typed filters, authenticated snapshot queries and AppSession access; verify unclaimed sources, deterministic matching, app source/action/credential boundaries, scope independence, tampering and metadata-only behavior.
- [x] 1.3 Document native query semantics and run focused core/query and policy regression tests; inspect and commit this verified implementation slice with active change artifacts excluded.

## 2. CLI and consumer flow checkpoint

- [x] 2.1 Add query-sources and trusted set-source-visibility commands; verify anonymous/app parity, strict inputs, structured errors and administration boundaries through executable CLI tests.
- [x] 2.2 Document and exercise register/find/ensure/lease for a simple unrestricted app and public publication/withdrawal, proving discovery does not grant acquisition or claim access.
- [x] 2.3 Run formatting, all-target tests, strict Clippy and OpenSpec validation; inspect and commit the verified CLI/docs slice, leaving other changes, mock, main specs and external repositories untouched.

## Verification evidence

- Native checkpoint: `cceadc7`; 10 query unit tests and 9 claim/policy integration tests passed.
- CLI checkpoint: `0f6ebe2`; executable source discovery, app credentials/policies, publication and real run-claim consumer flow passed.
- `cargo test --offline --all-targets`, `cargo fmt --check`, and `cargo clippy --offline --all-targets -- -D warnings` passed on Windows x86_64. The existing ignored native-keyring fixture remains ignored in the all-target run; native CLI fixtures exercise the real keyring. No new platform/MSRV verification is claimed.
- `openspec validate --all --strict`: 16 passed, 0 failed. Planning artifacts remain outside implementation commits; no sync/archive performed.
