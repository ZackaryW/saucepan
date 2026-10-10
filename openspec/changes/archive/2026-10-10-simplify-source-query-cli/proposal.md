# Proposal

## Why

Source queries already support anonymous callers, but the mandatory JSON file and registration-first examples make simple discovery look like it requires both an app identity and a saved request. Callers should be able to search from the command line immediately, adding app identity or a saved query only when needed.

## What Changes

- Make `saucepan query-sources` a complete query with no root override, app UUID, scope, credentials or query file.
- Default all CLI operations to the user's home `.saucepan` directory; retain explicit `--root PATH` precedence and fail with initialization guidance rather than creating a store during reads.
- Add optional exact `--uuid`, `--type`, `--repo`, `--origin` and `--url` filters using the existing source-query semantics.
- Retain `query-sources FILE` as optional saved-query input; reject file-plus-filter combinations rather than introducing hidden precedence.
- Lead help and documentation with anonymous, file-free discovery, then explain optional app-bound queries and the separate claim ownership flow.
- Preserve structured results/errors and verify parity among direct flags, saved queries and native query objects.

## Capabilities

### New Capabilities

- `source-query`: Add CLI input and onboarding requirements to the capability introduced by the implemented, not-yet-synced `add-source-discovery` change. Reuse its capability path; do not create a second source-discovery capability.

### Modified Capabilities

- `store-layout`: Add default user-home root selection alongside the existing explicit-root isolation requirement. This change also adds requirements to the pending `source-query` capability; sync `add-source-discovery` before this extension when eventually archiving.

## Impact

Primary implementation areas: `src/cli/mod.rs`, CLI input/root modules, `tests/source_query_cli.rs`, CLI regression tests, `docs/source-query.md` and README examples. Reuse the existing query model, authentication and policy evaluation. No new dependencies, legacy schema support, working-directory root search, claim-query redesign, anonymous ownership, external repository work, mock rewriting or sync/archive is included.
