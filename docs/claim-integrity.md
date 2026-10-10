# Claim integrity implementation notes

The native persistence API now supports explicit `IntegrityMode::Basic` through
`Store::create_with_integrity_and_keys`. Basic records use SHA-256 checksums with
an explicit contract and mode, and never call the supplied key provider. Catalog
membership, exact payload framing, identity/generation checks and journal replay
remain mandatory. `Store::integrity_mode()` reports the opened mode. A checksum
detects inconsistency, not a writer who recomputes every affected checksum.
The existing protected envelope is always interpreted as Keyring; opening never
downgrades it or rewrites it just to add a mode. Initialization reserves identity
and mode and rejects a retry selecting another mode.

CLI initialization and `Store::create` still default to protected custody pending
the basic-default integration. Explicit basic native stores support independently
optional application credentials. The protected-mode details below remain valid.

Native keyring operations are serialized across threads and Saucepan processes
using a coordination lock in the user's temporary directory (UID-qualified on
Unix). The lock contains no secrets. New key writes are read back before store
initialization succeeds. This addresses an observed Windows concurrent-key
creation failure; both thread and subprocess isolation tests are in
`tests/native_keys.rs`. A missing existing key still fails without replacement.

The record format is readable JSON with HMAC-SHA256 verification. The
HMAC key lives separately in OS keyring. This detects changes made without the
key; it does not conceal metadata or prevent filesystem edits. OS permissions
remain responsible for file access. A key holder can generate valid tags, and
replaying a complete earlier valid store requires an external anchor to detect.

Fresh stores use a signed `root.json` catalog. Every source, app, output, registry,
claim member and lifecycle receipt is signed. Claims live under
`claims/<app-uuid>/`; application records contain no embedded claim map.
Initialization provisions one store key through native Rust keyring APIs. A
reserved initialization identity supports retry after key-service failure.
Opening a live store never creates a replacement key or accepts unsigned data.

Each envelope binds store UUID, record kind, owner/member UUIDs and generation to
its exact raw JSON payload. Header fields are length-delimited; verification uses
constant-time HMAC comparison. Unknown/duplicate envelope fields and oversized
records are rejected. The bound is 16 MiB for each complete envelope, including a
transaction journal. A registry lists member-envelope digests; the root catalog
lists all live metadata references. Missing or changed records halt access and
reclaim. JSON editing is not an administrative update API.

A signed journal publishes member writes/deletions, app revision/binding, source
and output state, and lifecycle receipts under one store lock. Content publication
verifies its saved manifest before the metadata generation becomes live. Reopening
replays interrupted publication idempotently. Readers take the same lock, including
nested native API reads. Atomic file replacement is not a whole-filesystem
power-loss durability guarantee.

Application tokens are a separate opt-in control. `AppSession` carries identity
and proof through operations; source/scope/action restrictions still apply when
proof is disabled. Rotation invalidates old proof without revoking existing native
leases. Signed maintenance and registered shims can finish committed intent without
client tokens. The shim verifies its owned launcher/configuration digests and the
signed active binding before launch.

New credential policies use the `bearer` variant. Its verifier is HMAC-SHA256
keyed by a random 32-byte bearer token over framed domain, store, app and credential
generation fields. It needs no metadata signing key. The saved verifier cannot be
presented as the token. Existing `required` policies keep their previous protected
verification algorithm until explicit rotation; basic stores reject that variant.
This is high-entropy bearer proof, not password hashing. In basic mode, credentials
and policy restrictions enforce API behavior but cannot prevent a filesystem
writer from replacing metadata and recomputing checksums.

Production selects OS keyring. Tests explicitly inject `MemoryKeys` or native
fixtures that retire their own keys. No plaintext key fallback, Python binding,
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
