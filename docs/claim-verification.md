# Claim registry verification

Verified on Windows x86_64 with Rust 1.98.1 on 2026-10-09. The package remains
Rust-only: native keyring, HKDF/HMAC, serde and native lifetime guards; no Python
runtime binding or metadata encryption was introduced.

The counts and digests below belong to the claim-registry checkpoint, before
source discovery and its CLI improvements. See the [documentation map](README.md)
for subsequent capabilities, current check commands and remaining evidence gaps.

## Integrated fresh-root acceptance

Run `cargo test --offline --test claim_acceptance -- --nocapture`. The test uses
the actual built `saucepan` and `saucepan-shim` binaries in a disposable root,
with native OS key custody. It prints a JSON evidence record without credentials.
Set `SAUCEPAN_ACCEPTANCE_REPORT` to an explicit file path to save that record.

The recorded run used build `0.1.0+dev` with binary SHA-256
`9329d2b09af5b378578d0b1df6184996673ce895e730f2081eb30da5ff30ee4f`
and shim SHA-256
`49ec618439efefa1a72098c7f2d2a4bd1362d2bf60ca9ad08276d7abbc2bdefb`.
Its temporary fixtures were removed after verification.

| Observation | Recorded result |
| --- | --- |
| Consumer registration | `73e65d6a-5e61-4c65-895d-e8a8cb0d8612` |
| Independent claims | `c0dcf3a7-72c6-4191-9e9e-93a82f2d1dab`, `3e91010d-e4d2-4bd5-9812-a51283c5398f` |
| Original shared output | `63319abc-a82f-4c76-a5ba-c7c13c151b70` |
| Exact replacement | `2cc911de-ae08-4409-afa1-2b6cc2dc9c45` |
| Refreshed replacement | `aa568e17-a80d-4fd3-aedd-469654da60ab` |
| Managed app | `e4d7bd9b-9685-4ed8-8217-9417126b0226` |
| Original/rollback binding | `77ff38be-a71c-4415-806a-92c27dee38dd` |
| Updated binding | `86d09a5b-4d95-4e67-a734-72c32d5ac77c` |
| Uninstall | Complete; live registry retired |
| Final retention | Package keep 1; executable keep 2; no deferred/pending deletions |
| Historical mock | All 96 files retained identical SHA-256 hashes |

The same root covers exact/covered retries, scoped shared outputs, context lookup,
public/private claim discovery, forbidden-source and unowned cached-UUID denial, native
lease reclaim deferral, readable-member tampering, opt-in credentials, exact and
refresh recovery, unattended protected shim launch, managed update/rollback and
uninstall. An authenticated source publication interrupted after its journal was
written replays on open to the intended TTL of 43,200 seconds. The normal source
default remains 86,400 seconds; that value is changed only in the crash fixture.

This fixture uses local source inputs, including the actual built executables.
It does not rerun the historical two-build GitHub release/bootstrap harness.
Current source-definition discovery and default-root behavior have separate
fixtures in `tests/source_query_cli.rs` and `tests/cli.rs`.

## Scenario evidence

| Contract | Executable evidence |
| --- | --- |
| Crypto, credential bytes, strict models and policy evaluator | Library crypto/policy tests; `claim_contracts`, `errors` |
| Signed identities/membership, initialization retry, crash boundaries and refusal of old layouts | `metadata_integrity`; library index/claim/lifecycle fault tests |
| Optional proof, rotation, authentication before private claim lookup, restrictions and public discovery | `registry_security`, `claim_cli` |
| Registration/ensure contention, policy defaults, folder coverage and mutation races | `claim_ensure`; library claim ensure tests |
| Recovery providers, TTL, missing/changed bytes, dependency mismatch and revision conflicts | `claim_recovery`; library claim recovery tests |
| JSON errors, arguments, secret destination failure, lifecycle exit 2 and child streams | `claim_cli`, `cli`, `run` |
| Two project scopes and native view lifetime across real consumer exits | `scoped_consumer`, `access`, `run` |
| Protected shim, update/rollback/uninstall and receipt behavior | `lifecycle`, `managed`, integrated `claim_acceptance` |
| Concurrent native store-key custody | `native_keys` thread and subprocess tests |

## Host limits

Final checks passed: `cargo test --offline --all-targets` (144 top-level tests,
plus one nested fixture pass; the opt-in native keyring test passed separately),
`cargo fmt --check`, `cargo clippy --offline --all-targets -- -D warnings`, and
`openspec validate --all --strict` (15 items, no failures). Historical verification
documents describe their earlier checkpoints; they do not establish schema
compatibility with this implementation.

Rust 1.89 is declared and dependency metadata was audited, but that compiler is
not installed here and was not run. Windows x86_64 compilation, native keyring,
credential-file ACLs, process lifetime tests and actual shim launch were executed.

Cross-target checks were attempted with the installed Rust targets. Linux musl
was blocked by missing `x86_64-linux-musl-gcc`, macOS ARM64 by missing `cc`, and
Windows ARM64 by missing `clang` during dependency compilation. These are not
successful cross-platform checks; runtime behavior on those hosts is unverified.
Registry DNS was unavailable earlier, so builds and tests used cached dependencies
with `--offline`.

HMAC detects modified records without hiding them. Same-user filesystem access,
a signing-key holder, and complete valid-store rollback remain outside the
integrity guarantee. Each metadata envelope/transaction is bounded at 16 MiB.
The existing `mock/` installation is historical evidence, not an accepted new
store. No legacy decoder, migration or schema-version inheritance is provided.
