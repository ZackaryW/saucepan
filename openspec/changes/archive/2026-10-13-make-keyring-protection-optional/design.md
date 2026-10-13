# Design

## Context

See proposal.md for motivation. `Store::create/open` currently select `NativeKeyring` unconditionally. `Metadata` holds one `Authenticator`, which both authenticates persisted records and derives application credential verifiers. Catalog membership, record digests, bounded parsing, mutation locks, and recovery are already centralized in `core/claim/persistence.rs`. This is the correct boundary for adding a second integrity mode rather than scattering keyring bypasses across operations.

The CLI's `Init` command has no mode option. Integration fixtures in `tests/native_root/mod.rs` provision and retire OS keys; some invoke the manager and shim as separate executables. The ordinary suite also includes native-keyring-specific tests. Current CI provisions Linux Secret Service for the entire suite. macOS logs show `key_unavailable`; the exact native cause remains unresolved.

## Goals / Non-Goals

**Goals:** make the same runtime usable without a credential service; keep one transactional metadata pipeline; preserve independently configurable credentials and policies; provide honest, separately selectable native verification.

**Non-Goals:** encryption, a substitute OS access-control system, local plaintext signing keys, per-record integrity modes, automatic mode changes, a migration command, restoration of pre-reimplementation schemas, or fixing macOS ACL setup in this change. Compile-time removal of the keyring crate is also outside scope: runtime optionality is sufficient for deployment without a configured service.

## Decisions

### 1. Select mode once at initialization

Introduce `IntegrityMode::{Basic, Keyring}` in `models/claim/security.rs`. `saucepan init` and the simple native `Store::create` select Basic; `saucepan init --integrity keyring` and an explicit native creation option select Keyring. Initialization output exposes the mode, and native callers can inspect the opened mode. Routine commands and shims discover it from the selected root, not environment variables or command defaults.

An interrupted initialization records both reserved store identity and mode under the existing initialization lock. A retry must match the reservation; it cannot switch mode because a service is unavailable. Existing protected initialization markers containing only a UUID remain protected. Ordinary open never creates or repairs missing keys.

Alternative rejected: automatic fallback after keyring failure. That would make a temporary outage change the store's integrity guarantees.

### 2. Use one integrity interface with two explicit codecs

Refactor the existing record boundary into an integrity context with the store UUID, selected mode, and optional protected signing material. Both modes keep identity, generation, bounded raw payload parsing, catalog digests/membership, transaction parent checks, and validation-before-use. Basic mode uses SHA-256 over a domain-separated, length-framed representation of contract, mode, store UUID, identity, generation and exact payload bytes. It has no secret and never calls a key provider, including during cleanup or recovery.

Basic envelopes carry an explicit contract/mode discriminator and a checksum. Protected current-format envelopes retain their HMAC representation and verification inputs. Recognize that exact existing protected shape as Keyring; absence of a new mode field is never interpreted as Basic. New protected stores may use that existing protected codec, so no mass rewrite is required. Basic and protected envelopes are disjoint strict formats, and records or transactions of the wrong mode are rejected. Unknown contracts/modes produce `unsupported_store`; known-format integrity failures produce `integrity_error`.

Keep mode dispatch at the root-open boundary, then verify every member through the selected context. Do not attempt a second codec after verification fails. `KeyProvider` remains an explicitly injected dependency only for protected stores; existing protected fixture APIs remain available.

Alternative rejected: a constant HMAC key or a signing key stored beside JSON. Neither gives basic mode a meaningful custody guarantee and both obscure what is actually being checked.

### 3. Separate bearer proof from metadata protection

Preserve 32-byte OS-random bearer tokens. New credential records explicitly select a bearer verifier scheme independent of the store signing key: HMAC-SHA256 keyed by the random bearer token over a length-framed domain label, store UUID, app UUID and credential generation. Only the resulting verifier is persisted; verification uses the existing constant-time HMAC library primitive. This is a high-entropy bearer-token verifier, not password hashing. Token generation, secret destinations, rotation, registration retry behavior and policy enforcement remain unchanged.

Existing protected records with the current `Required` verifier retain their existing derived-key verification until an explicit credential change. Add a distinct serialized variant for new bearer verifiers rather than guessing which algorithm matches. New credential creation in either store mode uses the independent scheme. Basic stores reject the old signing-key-dependent variant; they never invoke keyring to service it. There is no silent credential reissue or algorithm fallback on failed proof.

Source/action/scope restrictions and public-discovery settings continue through the existing policy evaluator. Choosing Basic changes neither application identity nor permission. In Basic mode, a filesystem writer can rewrite policies and verifiers with matching checksums; credentials enforce API behavior and do not claim protection against that writer.

### 4. Keep package responsibilities narrow

| Area | Responsibility |
|---|---|
| `models/claim/security.rs` | Integrity mode and explicit credential verifier variants |
| `core/claim/security.rs` | Record codecs, integrity context, credential framing; native custody remains separate from policy |
| `core/claim/persistence.rs` | Root classification, mode-preserving reservation, snapshots and journal publication/replay |
| `core/mod.rs`, CLI init | Creation options, mode reporting, automatic open dispatch |
| `core/query`, lifecycle, launch/shims | Consume verified snapshots; no per-operation bypass switches |
| `utils/crypto.rs` and existing digest utilities | Byte-oriented SHA-256/HMAC operations; no store or credential policy |

Utility assessment: existing HMAC verification, randomness and SHA-256 dependencies cover the design. Reuse them; if a streaming SHA-256 helper is extracted, keep its input/output contract byte-oriented with known-vector and read-error tests. Record framing and token context binding are application contracts, not a new generic framework. No dependency upgrade, new cryptographic construction library, or Python runtime is required.

### 5. Split ordinary behavior tests from native custody

Make ordinary integration fixtures create Basic stores, including CLI children and real shims. Keep deterministic HMAC, missing-key and cross-mode tests using explicit in-memory/failing providers. A provider that panics or counts calls proves Basic never touches custody. Exercise both modes at the persistence boundary and use Basic for the full real-process lifecycle suite.

Group actual native-keyring tests behind explicit test selection (ignored dedicated tests with documented exact invocations). Include native round-trip/cleanup, concurrent creation, CLI-created stores opened by another executable, and a protected manager/shim flow. Selecting them must fail on unavailable custody; never turn an error into a passing test or automatically skip it. Give native subprocess checks bounded deadlines and capture stage/error diagnostics without secret bytes. This directly preserves coverage of the cross-executable failure that motivated this change.

Add `check_keyring` defaulting to false to manual/reusable platform checks and forward an explicit manual-release choice. Tag releases run the full ordinary platform matrix without requiring a credential service. Native checks are an additional explicitly selected lane within those selected platforms; their results remain visible and, when selected, block release on failure. Set up Linux Secret Service only for those checks; retain Git LFS for ordinary source tests. Do not introduce push/PR triggers or extra matrix jobs merely to report selection. Preserve platform selection, exact release SHA, MSRV controls, test output streaming, and release build dependencies. Report native tests as not run when omitted, never as verified. No hosted run is part of this planning action.

### 6. Explain the guarantees at the point of choice

Document Basic as consistency/corruption detection and Keyring as keyed modification detection. Both retain full payload verification when requested or required by activation/acquisition. Neither mode guarantees detection of complete rollback; Basic also cannot detect a coordinated rewrite of records, checksums and catalog. Mode labels in local metadata are not an external trust anchor against replacing the entire root. “Never downgrade” means no runtime fallback, conversion, or acceptance of mixed-mode protected state, not protection against arbitrary wholesale filesystem replacement.

## Risks / Trade-offs

- Basic mode is deliberately weaker against a filesystem writer -> make mode visible at initialization and describe its limits; do not claim checksum authenticity.
- An integrity abstraction could weaken recovery -> run the same corruption, identity, interrupted-publication and replay tests for both modes before wiring the default CLI path.
- Existing protected credentials use the signing key -> retain their explicit old variant only in protected stores; prove rotation to the independent variant without changing tokens on open.
- Hiding native tests could hide product regressions -> keep explicit cross-process/shim checks, bounded failures and documentation of which platforms were actually verified. This change does not certify macOS native custody.
- Strict older binaries cannot read Basic envelopes or new credential variants -> ship matching manager/shim builds, retain rejection behavior, and do not promise old-reader compatibility.

## Migration Plan

1. Add mode-aware persistence and independent credential contracts with focused tests, while keeping existing production defaults until the foundation passes.
2. Integrate default Basic initialization, explicit protected creation, CLI/shims and basic process fixtures together as a verified checkpoint.
3. Separate native test selection and reconcile workflows/docs/spec references with integrated acceptance evidence.

Existing current-format protected stores continue to open as protected with their original keys and existing credentials; no records are rewritten merely by opening. Earlier unsigned, encrypted and inline-claim layouts remain rejected. Do not alter historical `mock/`. Use disposable fresh roots for Basic and protected acceptance. Changing an initialized store's mode is unsupported in this change; users needing another mode create a separate root. Keep the current build for Basic/new-credential roots if rolling back the software would otherwise require an older incompatible reader; no reverse conversion is provided.
