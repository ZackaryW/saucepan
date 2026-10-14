# Claim integrity implementation notes

Relocated from `docs/claim-integrity.md`. These notes combine the original
cryptography checkpoint with later integrity changes. Dependency versions and
verification results describe their stated checkpoints; consult the current
specifications and manifests for the supported contract.

Immutable policy records participate in the same verified catalog and journal as
claim metadata. Adopted claim evidence is reconstructed from definition UUIDs and
digests, then checked against the current source/application constraints. Editing
an effective value alone is invalid. Removed attachments do not delete definitions,
so historical output evidence remains resolvable. Current restrictions and
credentials are checked separately from that historical evidence.

In-flight operations observe source attachment revisions and application revisions.
Lifecycle preparation also persists its source observation, so restarting an
operation cannot bypass an intervening adoption. Unrelated policy registration
does not invalidate these observations. Optional native custody is not required by
the policy fixtures; they use basic integrity or injected deterministic keys.

`saucepan init` and `Store::create` default to `IntegrityMode::Basic`. Select
protected custody with `saucepan init --integrity keyring` or
`Store::create_with_integrity(root, IntegrityMode::Keyring)`.
`Store::create_with_integrity_and_keys` accepts an explicit test provider.
Basic records use SHA-256 checksums with
an explicit contract and mode, and never call the supplied key provider. Catalog
membership, exact payload framing, identity/generation checks and journal replay
remain mandatory. `Store::integrity_mode()` reports the opened mode. A checksum
detects inconsistency, not a writer who recomputes every affected checksum.
The existing protected envelope is always interpreted as Keyring; opening never
downgrades it or rewrites it just to add a mode. Initialization reserves identity
and mode and rejects a retry selecting another mode.

Both modes support independently optional application credentials. Routine commands
and shims follow the persisted mode without flags. There is no in-place conversion.

Native keyring operations are serialized across threads and Saucepan processes
using a coordination lock in the user's temporary directory (UID-qualified on
Unix). The lock contains no secrets. New key writes are read back before store
initialization succeeds. This addresses an observed Windows concurrent-key
creation failure; both thread and subprocess isolation tests are in
`tests/native_keys.rs`. A missing existing key still fails without replacement.

The protected record format is readable JSON with HMAC-SHA256 verification. The
HMAC key lives separately in OS keyring. This detects changes made without the
key; it does not conceal metadata or prevent filesystem edits. OS permissions
remain responsible for file access. A key holder can generate valid tags, and
replaying a complete earlier valid store requires an external anchor to detect.

Stores use a verified `root.json` catalog. Every source, app, output, registry,
claim member and lifecycle receipt uses the store's integrity mode. Claims live under
`claims/<app-uuid>/`; application records contain no embedded claim map.
Protected initialization provisions one store key through native Rust keyring APIs. A
reserved initialization identity supports retry after key-service failure.
Opening a live store never creates a replacement key or accepts earlier unsigned layouts.

Each envelope binds store UUID, record kind, owner/member UUIDs and generation to
its exact raw JSON payload. Header fields are length-delimited; verification uses
SHA-256 in basic mode and constant-time HMAC comparison in protected mode. Unknown/duplicate envelope fields and oversized
records are rejected. The bound is 16 MiB for each complete envelope, including a
transaction journal. A registry lists member-envelope digests; the root catalog
lists all live metadata references. Missing or changed records halt access and
reclaim. JSON editing is not an administrative update API.

A verified journal publishes member writes/deletions, app revision/binding, source
and output state, and lifecycle receipts under one store lock. Content publication
verifies its saved manifest before the metadata generation becomes live. Reopening
replays interrupted publication idempotently. Readers take the same lock, including
nested native API reads. Atomic file replacement is not a whole-filesystem
power-loss durability guarantee.

Application tokens are a separate opt-in control. `AppSession` carries identity
and proof through operations; source/scope/action restrictions still apply when
proof is disabled. Rotation invalidates old proof without revoking existing native
leases. Verified maintenance and registered shims can finish committed intent without
client tokens. The shim verifies its owned launcher/configuration digests and the
verified active binding before launch.

New credential policies use the `bearer` variant. Its verifier is HMAC-SHA256
keyed by a random 32-byte bearer token over framed domain, store, app and credential
generation fields. It needs no metadata signing key. The saved verifier cannot be
presented as the token. Existing `required` policies keep their previous protected
verification algorithm until explicit rotation; basic stores reject that variant.
This is high-entropy bearer proof, not password hashing. In basic mode, credentials
and policy restrictions enforce API behavior but cannot prevent a filesystem
writer from replacing metadata and recomputing checksums.

Ordinary fixtures use basic stores and reject custody calls. Protected tests inject
`MemoryKeys`; actual native custody tests require explicit selection and retire
their own keys. Basic-mode results do not verify native custody. No plaintext key fallback, Python binding,
legacy schema decoder, or schema-version inheritance is present. The earlier
`mock/` installation is historical and is not opened or migrated during tests.

## Portable primitives

`src/utils/crypto.rs` provides:

- `random_bytes<N>()`: OS randomness in a zeroizing buffer.
- `derive_key<N>(secret, salt, info)`: HKDF-SHA256; callers supply distinct purpose
  labels for record MACs and credential verifiers.
- `authenticate(key, reader)`: HMAC-SHA256 over the complete byte stream, failing
  if the reader fails rather than returning a tag for partial data.
- `verify(key, reader, tag)`: library constant-time verification with invalid
  tags, lengths, and changed input rejected.

Callers own record parsing, bounded input, identity/generation framing, key custody
and serialization. These helpers neither interpret JSON nor write files. The
previous experimental `seal`/`open` helpers and AEAD dependency were removed.

## Dependencies

| Package | Pinned version | Declared minimum Rust |
| --- | --- | --- |
| sha2 (crypto alias) | 0.11.0 | 1.85 |
| hkdf | 0.13.0 | 1.85 |
| hmac | 0.13.0 | 1.85 |
| getrandom | 0.4.3 | 1.85 |
| zeroize | 1.9.1 | 1.85 |
| keyring | 4.2.0 | 1.88 |

All listed packages are MIT OR Apache-2.0. Existing content hashing remains on
sha2 0.10; `sha2_crypto` uses the digest generation required by hmac/hkdf 0.13.
This avoids changing unrelated content hashing during the utility checkpoint.

The explicitly selected keyring `v1` feature uses Windows Credential Manager,
macOS Keychain, or Secret Service on supported Unix hosts. Cached keyring source
was inspected for `Entry::new`, `set_secret`, `get_secret`, and
`delete_credential`. The application uses these native Rust APIs; there is no Python binding.

## Verification on 2026-10-09

Windows x86_64, Rust 1.98.1:

- `cargo test --offline --lib utils::crypto`: five tests passed, including RFC
  HKDF/HMAC vectors, changed keys/data/tags, empty/truncated input, random buffers,
  and reader failure propagation.
- `cargo fmt --check` and strict library clippy passed after formatting.
- Cargo resolved the dependencies using its Rust 1.89 compatibility fallback;
  the Windows dependency metadata declared no minimum above 1.89.

Rust 1.89 itself is not installed and was not executed. macOS/Unix builds and
runtime keyring operations on those hosts were not executed. The Windows native
keyring round trip and cleanup passed. Registry DNS resolution failed;
this checkpoint used locally cached dependency source and offline compilation.

Additional cutover verification uses `tests/metadata_integrity.rs` (identity/tag
substitution, missing records, interrupted publication, key-service failures and
strict unsigned-root refusal), `tests/registry_security.rs` (actual app sessions,
optional proof, rotation, independent policy restrictions and public discovery),
and the claim/query/access/lifecycle regression suites. The protected-shim lifecycle
test exercises unattended launch, edited metadata and sidecars, and a native lease
remaining usable after uninstall.
