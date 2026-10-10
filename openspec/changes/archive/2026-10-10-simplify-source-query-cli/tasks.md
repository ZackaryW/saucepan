# Tasks

## 1. File-free source query checkpoint

- [x] 1.1 Make source-query file input optional and add exact UUID/type/repo/origin/URL flags converging on the existing query object; verify empty input and each flag against equivalent native and saved-file queries in executable CLI tests.
- [x] 1.2 Reject file/filter combinations and repeated or invalid filters without precedence or file reads; verify structured invalid_request errors, no success stdout, missing-file conflict handling, string bounds and contradictory valid filters.
- [x] 1.3 Verify no-app queries against mixed public/private unclaimed sources and an initialized store without any apps; verify explicit app source/action restrictions and credential errors with no anonymous fallback, preserving metadata and claim state.
- [x] 1.4 Default all CLI commands to user-home .saucepan with explicit --root precedence; verify missing-root initialization guidance, no automatic creation, working-directory independence, invalid-home handling and explicit override isolation in disposable-home tests.
- [x] 1.5 Lead query help, README and source-query documentation with anonymous/no-file/no-root-override examples, then flags, explicit roots, optional app identity and saved files; verify help output and run the documented paths through the CLI fixture.
- [x] 1.6 Run source-query and existing CLI regression tests, cargo fmt --check, strict all-target Clippy and strict OpenSpec validation; inspect the scoped diff and create one coherent implementation checkpoint when commits are authorized, excluding active OpenSpec artifacts. Do not sync/archive or edit the base change.

## Verification evidence

- Implementation checkpoint: `ee02f90`.
- Focused CLI/source-query/credential tests and root resolver unit test passed, including isolated child-process homes and literal filter/file parity.
- `cargo test --offline --all-targets`, `cargo fmt --check` and `cargo clippy --offline --all-targets -- -D warnings` passed on Windows x86_64. One pre-existing native-keyring test remains ignored in the all-target run; no cross-platform/MSRV verification is claimed.
- `openspec validate --all --strict`: 17 items passed. Main specs, historical mock and other change artifacts were not edited. No sync/archive was performed; active planning artifacts remain excluded from the implementation commit.
