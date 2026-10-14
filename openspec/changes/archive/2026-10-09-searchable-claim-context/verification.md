# Query checkpoint verification


The counts below describe the query checkpoint before the separate claim registry.
Current integrated evidence is in [claim-verification.md](../2026-10-10-secure-application-claim-registry/verification.md).

Verified on 2026-10-09 on Windows x86_64 with Rust 1.98.1:

- `cargo test --all-targets`: 96 top-level tests passed (97 reported passes
  including a nested child fixture run). Coverage includes context publication,
  replacement, recovery, update/rollback conflicts, query filtering and snapshots,
  portable string matching, and the CLI example flow.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: passed.
- Strict OpenSpec validation: this change and all 13 existing main specs passed.
- SHA-256 comparison: all 96 existing `mock/` files remained unchanged.

Linux/macOS execution and the declared minimum Rust 1.89 toolchain were not run
for this change. String-matching semantics are specified portably and covered by
platform-neutral cases, but this run establishes Windows execution evidence only.
