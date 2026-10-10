# Optional keyring verification

Verified on Windows x86_64 with Rust 1.98.1 on 2026-10-10. New stores use basic
checksum integrity; protected metadata and independently optional app credentials
remain supported. This report supersedes earlier setup instructions requiring a
keyring for every store, without changing those reports' historical results.

## Release-to-lifecycle acceptance

Executed `uv run --no-project python -B scripts/lifecycle_acceptance.py` using two
actual optimized manager/shim builds, a loopback GitHub release fixture and a fresh
basic root. The development harness is not a runtime dependency. The updated recipe
supplies explicit claim scopes and inspects separate claim records. After app
retirement, it retries the removal receipt through administrative `reconcile`,
rather than trying to recreate an app session for `uninstall`.

The successful run recorded 29 operations: release bootstrap, shared acquisition,
activation and a live target, successful and failed self-update, rollback,
deactivation/reactivation, shim replacement, shared-output uninstall, pending
Windows cleanup, reconciliation and idempotent receipt retry, final reclaim, and
generic installation/removal. All 96 historical `mock/` files remained unchanged.

| Build | Manager SHA-256 | Shim SHA-256 |
| --- | --- | --- |
| lifecycle-a | `5daf011a5de4d638392952b848abfbf9547fd612b755732f46096c208830abf5` | `52edaa96c448518030f9909d3d11812f3ae2db3c1cc4b3be4c7edef5d40a556c` |
| lifecycle-b | `70ed784597e9aae786e4874a444bc412035a15bd80ba48cba428ea73815a0fbb` | `b0551a4a50f78970373aa3b382f58652c3c2905b62da8789360622a6aa949d14` |

The local machine-readable report is
`target/lifecycle-acceptance/8a46a36c-6d3b-4667-b3ad-844848fde5f5/report.json`.
Its root and outputs are disposable evidence, not the user's installation.

## Integrity and native checks

Focused tests exercise both basic and injected protected persistence: exact-byte
checks, catalog membership, mode separation, interrupted initialization and
publication replay. Credential tests cover policy restrictions in both modes,
rotation without revoking leases, and opening existing protected credential records
without reissue. Basic fixtures reject custody calls rather than silently faking a
native service.

The explicitly selected Windows native checks passed: key round-trip and cleanup,
concurrent thread/process creation, and a protected store shared by the actual
manager and shim. Removing its native key caused CLI access to fail without
changing the protected root. A deterministic worker-failure test verifies failures
propagate instead of becoming successful skips; diagnostics expose controlled
operation/reason fields without backend secret payloads.

The full Windows `cargo test --locked --all-targets -- --nocapture` suite passed.
Its four native-service tests were ignored by default and passed when selected
separately as described above.

Formatting, locked all-target Clippy, five release-packaging tests, workflow
actionlint and strict OpenSpec validation passed. Workflow selection was checked
for basic Linux, selected macOS custody, tag releases and manual protected releases.
No hosted workflow was dispatched. Linux/macOS runtime and Rust 1.89 were not
executed here. In particular, the recent macOS native-keychain failure remains
unresolved; basic-mode success is not evidence that native custody works there.

Current commands:

```sh
cargo test --locked --all-targets -- --nocapture
cargo test --locked --test native_keys -- --ignored --nocapture
cargo test --locked --test metadata_integrity native_keyring_round_trip_and_cleanup -- --ignored --exact --nocapture
uv run --no-project python -B scripts/lifecycle_acceptance.py
```

The first command uses no native custody. The next two require a configured native
service. The last builds two release variants and runs the isolated basic flow.
Basic checksums do not protect against coordinated metadata/checksum rewriting;
neither mode provides an external rollback or whole-root replacement anchor.
