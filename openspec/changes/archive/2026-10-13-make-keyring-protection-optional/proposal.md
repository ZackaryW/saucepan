# Proposal

## Why

Saucepan currently requires an operational OS keyring to create or open any store, even when application credentials are disabled. Recent macOS integration failures expose this deployment dependency: ordinary caching, querying, and managed launch should work without a credential service, while stronger metadata protection remains an explicit option.

## What Changes

- **BREAKING default behavior:** new stores default to `basic` integrity, using checksummed readable metadata without a signing key. Explicit `keyring` mode retains native-keyring HMAC protection.
- Persist the store mode; opening, recovery, and shims follow that mode without repeating flags. Existing current-format protected stores stay protected, and unavailable keys never trigger fallback or replacement.
- Keep app credentials independently opt-in in both modes by separating new bearer-token verification from the store signing key. Preserve source, action, scope, public-discovery, and claim restrictions in both modes.
- Preserve record identity, catalog membership, revision checks, atomic publication, content verification, leases, retention, and lifecycle behavior. Basic checksums detect inconsistency, not deliberate rewriting of metadata and checksums together.
- Run ordinary integration and real-build acceptance without native keyring access. Isolate explicit native-keyring integration checks; report unavailable native verification accurately and retain manual/release-only CI and platform selection.
- Reconcile current documentation and dependent specs so “verified” does not imply HMAC in basic mode. No encryption, plaintext signing-key fallback, automatic conversion, or pre-reimplementation compatibility is introduced.

## Capabilities

### New Capabilities

None; extend existing integrity and store contracts.

### Modified Capabilities

- `claim-security`: explicit integrity modes, independent credentials, native custody opt-in, verification boundaries and test separation.
- `store-layout`: initialization mode, persistent discrimination, and mode-aware readable records.
- `content-verification`: verify the selected metadata integrity scheme independently of payload scan level.
- `source-query`: mode-aware verified source snapshots and visibility updates.
- `claim-query`: mode-aware verified claim snapshots.
- `lifecycle-recovery`: consistent integrity mode for journals, receipts, and replay.
- `application-lifecycle`: publication guarantees apply in both modes.
- `shim-maintenance`: verify ownership using the store's integrity mode.
- `self-management`: real-build acceptance in a keyring-free basic store and separate protected verification.

## Impact

Primary changes are in `src/core/claim/{security,persistence}.rs`, `src/models/claim/security.rs`, `src/core/mod.rs`, CLI initialization, and consumers of credential verification. Reuse `src/utils/crypto.rs` and existing digest primitives; retain Rust-only native custody. Test fixtures, native-keyring tests, manager/shim interoperability coverage, platform/release workflow inputs, README, integrity/release documentation, and current spec wording need reconciliation. Historical `mock/` and archived changes remain untouched. This proposal does not claim the macOS native-keychain failure is fixed.
