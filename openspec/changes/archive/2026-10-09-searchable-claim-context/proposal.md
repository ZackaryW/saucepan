# Proposal

## Why

Consumers can address claims by UUID, but cannot discover their existing claims using project, scope, or source-alias metadata. Searchable claim context lets one application maintain independent selections for several consumer projects while continuing to share immutable cached outputs.

## What Changes

- Add optional plain string-to-string context to claims, with atomic context-bearing acquisition and explicit whole-context replacement for an existing claim.
- Add application-scoped queries with a flat `where` list of `{key, op, value}` conditions. Support exact equality and explicit glob matching; combine conditions with AND and return every match in deterministic order.
- Place query behavior and query models in `src/core/query/`, separate from index persistence. Keep context in application claim records, without new project directories or a persistent search index.
- Preserve context through primitive updates, managed updates, rollback, and recovery; context mutations participate in application revision checks.
- Expose context and querying through native APIs and JSON-file CLI inputs. Document that query results are metadata snapshots, not leases, ownership grants, uniqueness constraints, or automatic claims on future resources.

## Capabilities

### New Capabilities

- `claim-query`: Application-scoped context search, query validation, deterministic results, and native/CLI behavior.

### Modified Capabilities

- `application-claims`: Claim context creation, replacement, persistence, and preservation through existing transitions.

## Impact

- Models and persistence: `src/models/index/mod.rs`, `src/core/index/claims.rs`, acquisition publication, and existing lifecycle claim construction/recovery paths.
- New query modules: `src/core/query/{mod,model,matching}.rs`; a small domain-independent string-pattern utility for portable context matching without filesystem path normalization.
- CLI: scoped acquisition context, context replacement, and query commands in `src/cli/mod.rs`; focused native/CLI and lifecycle regression coverage, README and usage examples.
- No change to output identity, deduplication, eviction, source refresh, store layout, or resource lease guarantees. No inherited schema version, legacy migration, or general cross-build compatibility promise.
- Powerspec integration, Python SDK work, automatic project discovery, cross-application queries, nested expressions, wildcard mutations, and standing wildcard ownership are outside this change.
