# Proposal

## Why

The completed retrieval/acquisition split makes operations composable, but applications still embed access rules and policy defaults while claims embed overrides. That conflicts with the intended flat, UUID-linked policy model and makes policy reuse, adoption and explanations depend on several representations of authority.

## What Changes

- **BREAKING**: replace authoritative nested policy bundles in application, source, claim and operation contracts with references to standalone typed policy records. Preserve resource inputs such as source, selector, scope, subpath and destination as resource data.
- Give each policy an immutable UUID. A changed policy receives a new UUID; adoption is an explicit, revision-checked resource change. Repeated registration of identical records is idempotent, and references cannot silently change meaning.
- Expose direct policy lookup by UUID in native and CLI interfaces, returning the verified definition without requiring a query file or an application registration for trusted local inspection.
- Introduce one bounded resolver/evaluator for access restrictions, visibility, behavior defaults, permitted choices and operation selections. Preserve the distinction between choosing behavior and granting authority; request-supplied policies cannot broaden registered restrictions.
- Support persistent policy records and an ephemeral caller-supplied catalog. Inline temporary/no-store operations remain usable without a store, application, credentials or persistent policy registration.
- Cover existing retrieval caching/freshness, verification, release selection, acquisition updates, identical pulls, revision history, reclaim and usable-app protection. Preserve optional claims, separate scopes, deterministic claim reuse, source/output paths and lifetimes.
- **BREAKING**: establish a distinct flat-policy store contract and reject earlier nested-policy stores without migration or modification. Keep metadata integrity modes and credentials independent from policy selection; no encryption or new keyring requirement.

## Capabilities

### New Capabilities

- `policy-resources`: immutable typed policy definitions, reference resolution, explicit adoption, provenance, portable catalogs and lifecycle safety.

### Modified Capabilities

- `application-claims`: claims retain policy references and derived evidence; reuse and explicit policy adoption preserve claim identity and scope.
- `claim-security`: all operation paths evaluate resolved references under the existing local administration and opt-in credential boundaries.
- `output-policies`: replace embedded defaults/overrides with referenced choices and constraints while preserving revision and reclaim semantics.
- `source-resolution`: reference independent freshness, release filters and selection strategies without changing provider behavior.
- `source-query`: derive visibility from attached policy resources and preserve anonymous, scope-independent discovery.
- `store-layout`: add verified policy records and distinguish the flat-policy contract from unsupported earlier layouts.

## Impact

Extend existing `models/policy` and `core/policy`; adapt claim registry validation, source/store/retrieval orchestration, query, lifecycle, CLI, tests and consumer documentation. Reuse the existing metadata journal, integrity providers, locks and hashing utilities. No graph engine, external IAM service or new dependency is justified.

This change builds on the implemented but unsynced `compose-retrieval-and-acquisition`. Its behavioral guarantees remain the baseline; this change supersedes its inline policy spellings and convenience eviction adapter. Synchronize/archive that predecessor before synchronizing this change so its older deltas cannot restore nested contracts. Planning does not perform either archive.

Power Eagle integration examples will demonstrate flat references; editing its adapter, adding an IPC lease bridge, changing historical `mock/`, migrating user stores, or expanding CI triggers is out of scope.
