# Proposal

## Why

Applications can acquire registered sources by UUID but cannot find unclaimed sources. Claim discovery cannot serve as a source catalogue because ownership and public claim visibility are independent of source registration.

## What Changes

- Add metadata-only source queries with optional exact UUID, type, repository, origin and URL filters.
- Expose application queries through existing credentials, discover action and source allowances, without requiring a claim scope.
- Add explicit source public-discovery visibility, private by default, and trusted administrative configuration.
- Provide native and CLI interfaces, deterministic compact results, documentation and focused verification.

## Capabilities

### New Capabilities

- `source-query`: Search authenticated registered source metadata before ownership, subject to application permissions or explicit public visibility.

### Modified Capabilities

None. Provider resolution, claim queries, scope ownership, acquisition and leases retain their contracts.

## Impact

Touches `src/models/source`, `src/core/query`, source metadata registration/configuration, the existing policy evaluator integration, `AppSession`, CLI and tests/docs. No new dependency, external repository changes, legacy migration, source fetches, claim refactoring, main-spec synchronization or archiving is included. Builds on the implemented `secure-application-claim-registry` change without editing its artifacts.
