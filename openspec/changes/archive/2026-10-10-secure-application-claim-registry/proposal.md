# Proposal

## Why

Applications currently accumulate duplicate claims, keep claims inside general index records, and cannot register claim recovery or credential policies. Consumers also lack stable error codes, making reliable integration difficult. These concerns need one coordinated contract across acquisition, querying, recovery, and managed application operations.

## What Changes

- **BREAKING**: Move claims into `claims/<app-uuid>/`, with an application registry and separate claim records, implemented in `core/claim` and `models/claim`. Reject incompatible store layouts rather than inheriting an old schema or silently migrating them.
- Add explicit claim scopes and atomic ensure semantics: matching or fully covered requests reuse claims within a scope; different scopes update independently while sharing output content. Context remains searchable labels, separate from claim identity.
- Register application source/action/scope restrictions, public discovery exposure, policy defaults and allowed claim overrides, including typed `reclaim_policy` behavior for unavailable content. Keep recovery, output eviction, and source freshness distinct.
- Keep records as readable JSON and verify HMAC-SHA256 tags using a separate OS-keyring key before accepting them. Encryption is outside this change. Application credentials remain **opt-in**, independently of source and claim restrictions. Apply one caller/action/resource policy decision across app operations; generic discovery excludes non-public scopes. Recovery and launch must preserve integrity and policy boundaries.
- Add structured consumer errors to the Rust library and CLI, and integrate authorized claim views with existing native resource leases. Preserve the existing exact-output, lifecycle, and execution lifetime guarantees.

## Capabilities

### New Capabilities

- `claim-security`: HMAC-protected readable records, opt-in credentials, source/claim authorization, key custody, authenticated recovery, and constrained launch authority.
- `claim-recovery`: Registered `reclaim_policy` modes that distinguish exact restoration, explicit refresh, and reporting unavailable content.
- `consumer-errors`: Stable machine-readable error codes and consistent Rust library and CLI failure behavior.

### Modified Capabilities

- `store-layout`: Separate claim storage and explicit rejection of incompatible layouts.
- `application-claims`: Explicit scopes, request coverage, claim reuse, registry ownership, and context behavior.
- `claim-query`: Policy-filtered registry snapshots, explicit scope filtering, public-only generic discovery, and covered-request views.
- `resource-access`: Authorized claim leasing and subpath views with native lifetime protection.
- `output-policies`: Registered policy defaults and bounded claim overrides without altering source freshness or output identity.
- `application-lifecycle`: Coordinated registry, binding, and application transitions, including removal.
- `lifecycle-recovery`: HMAC-verified journals and receipts, generation consistency, and secret-free recoverable state.

## Impact

Affected areas include `src/core/index/claims.rs`, index transactions, acquisition, reclaim, query, activation, bootstrap, lifecycle receipts, shims, CLI dispatch, and the public Rust models. Add `src/core/claim/`, `src/models/claim/`, and portable crypto mechanics in `src/utils/crypto.rs`. The delivered package remains Rust-only; OS credential storage uses native Rust keyring packages.

Cryptographic and native keyring dependencies require compatibility checks against Rust 1.89 and supported hosts. Existing tests and examples must move to the new contract; preserve the historical `mock/` installation and use fresh isolated acceptance roots. The old repository supplies security implementation evidence, not a compatible schema. A Rust/CLI integration fixture exercising project-scoped resource consumption belongs here; migrating the Powerspec repository and restoring its old SDK protocol are outside this change.
