# Tasks

## 1. Durable managed setup and read-only inspection

- [x] 1.1 Add versioned setup/status models and integrity-covered setup records with root/input fingerprints, stable registration identities and linked lifecycle IDs; verify serialization, tamper rejection, current-store reopen and unchanged integrity-mode tests.
- [x] 1.2 Implement read-only setup-status routing before generic store opening, deriving absent/unmanaged/ready/pending/conflict from verified current ownership; verify actual CLI JSON/exit behavior for missing, healthy, damaged and unsupported roots without initialization or network effects.
- [x] 1.3 Implement exact-tag setup preparation using existing release profiles and bootstrap, durable shim staging and root serialization; verify fresh setup with built binaries against an isolated loopback release and concurrent setup without duplicate registrations or launcher ownership.
- [x] 1.4 Implement exact-input retries, linked lifecycle recovery and ready-installation reuse; verify interruption at registration/acquisition/publication boundaries, input conflicts, durable-path retry after scratch removal, and no-network same-tag reuse while different tags and damaged owned shims return explicit guidance.
- [x] 1.5 Document setup/status commands, result schemas, exit codes, retry instructions and metadata compatibility limits; verify every documented command against isolated fixtures and preserve existing low-level bootstrap behavior.

## 2. Explicit verified raw-manager migration

- [x] 2.1 Add migration preflight requiring an initialized supported root, explicit operation/consent/digest and an externally running manager; verify absent consent, wrong digests, links/hardlinks/reparse points, sidecars, ownership reservations and binary-only roots leave candidates unchanged.
- [x] 2.2 Implement persisted intent, exclusive backup creation, flush/digest verification, permission preservation and source recheck before displacement under launcher coordination; verify backup contents and permissions, no-overwrite behavior, changed-source refusal and unrelated-file preservation.
- [x] 2.3 Connect migration phases to setup recovery without automatic restoration over an occupied destination; verify interruption before/after backup and displacement, failed bootstrap, subsequent explicit retry, retained backups after success and busy Windows executable behavior on Windows.
- [x] 2.4 Document migration commands, availability gap, backup/retry evidence and separate-root/manual-preservation guidance; verify the documented flow using an isolated initialized store with a raw manager and record platform-specific checks actually run.

## 3. Explicit version updates and SDK contracts

- [x] 3.1 Add optional exact-tag self-update planning and fingerprinting while preserving omitted-tag behavior; verify source/scope/platform/policy invariants and changed-tag retry rejection with existing lifecycle tests.
- [x] 3.2 Commit the replacement request, selected output and binding together after validation under revision fences; verify failed probes and concurrent revisions retain the previous request/claim/binding, and successful explicit updates advance all three coherently.
- [x] 3.3 Add Python setup/setup_status methods, typed setup decoding and self-update tag forwarding; verify real argv/root/exit-0/exit-2 handling, malformed results, mutation uncertainty, app-bound rejection and no install/import/construction side effects in SDK tests.
- [x] 3.4 Add equivalent TypeScript methods and types with a separate setup decoder; verify contract parity, exit-2 receipts, malformed results, uncertainty, app-bound rejection and no automatic mutation retry in TypeScript tests.
- [x] 3.5 Update CLI and both SDK usage documentation for exact-tag changes and setup recovery; verify examples against the same built provider fixture and retain existing SDK setup/bootstrap regression coverage.

## 4. Official installers and release artifacts

- [x] 4.1 Add the PowerShell 5.1 installer using literal arguments, external staging, exact archive checksum/entry checks and setup result forwarding; verify spaces/Unicode, prerequisites, wrong checksums, duplicate/unsafe entries, unsupported targets, operation reporting and exit-2 receipt preservation with a local release fixture.
- [x] 4.2 Add the POSIX installer with the same public inputs and behavior using curl/unzip and available SHA256 tools; verify supported Unix execution, executable permissions, spaces/Unicode, checksum/entry rejection, missing prerequisites and unchanged stdout/exit behavior on available platforms.
- [x] 4.3 Extend release packaging, checksums, exact asset allowlists and workflow upload selection for both scripts; verify release tests reject missing/mismatched script tags and unexpected assets, accept the complete local artifact set and preserve selective/manual workflow behavior without publishing a release.
- [x] 4.4 Document installer invocation, supported platforms, prerequisites, explicit migration and PATH instructions; verify scripts require neither development runtimes nor SDK installation and reject v0.10.0 as setup-incapable without raw-copy fallback.

## 5. Integrated provider and consumer handoff

- [x] 5.1 Exercise actual built manager/shim, packaged installers and both SDKs through fresh setup, reuse, migration, interrupted retry and explicit tag update in isolated roots; record artifact hashes, stable identities, retained backups, launcher behavior and unchanged unrelated application data.
- [x] 5.2 Run repository-required Rust formatting/lint/test checks, both SDK suites and release packaging checks once the integrated change is complete; record results and any unavailable host/keyring coverage without claiming unexecuted platforms passed.
- [x] 5.3 Validate this change with openspec validate add-managed-setup --strict --no-interactive and deliver the versioned contract plus local artifact/SDK revision evidence to adopt-saucepan-managed-setup; verify all acceptance claims reference real evidence and keep release publication and production-home migration outside this implementation.
