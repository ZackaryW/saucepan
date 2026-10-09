# Claim integrity implementation notes

The planned record format is readable JSON with HMAC-SHA256 verification. The
HMAC key lives separately in OS keyring. This detects changes made without the
key; it does not conceal metadata or prevent filesystem edits. OS permissions
remain responsible for file access. A key holder can generate valid tags, and
replaying a complete earlier valid store requires an external anchor to detect.

Only the portable primitives and dependencies are implemented at this checkpoint.
Store records do **not yet** have HMAC envelopes, key custody, or the new unified
operation-policy evaluator. Application credential enforcement remains a planned
independent, opt-in control. Do not infer store tamper protection from the presence
of the helper module.

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
`delete_credential`. The application will use these native Rust APIs; there is
no Python binding. Actual production key provisioning belongs to the subsequent
store integration checkpoint.

## Verification on 2026-10-09

Windows x86_64, Rust 1.98.1:

- `cargo test --offline --lib utils::crypto`: five tests passed, including RFC
  HKDF/HMAC vectors, changed keys/data/tags, empty/truncated input, random buffers,
  and reader failure propagation.
- `cargo fmt --check` and strict library clippy passed after formatting.
- Cargo resolved the dependencies using its Rust 1.89 compatibility fallback;
  the Windows dependency metadata declared no minimum above 1.89.

Rust 1.89 itself is not installed and was not executed. macOS/Unix builds and
runtime keyring operations were not executed. Registry DNS resolution failed;
this checkpoint used locally cached dependency source and offline compilation.
