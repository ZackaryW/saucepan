# Proposal

## Why

Saucepan currently couples provider fetching, extraction, output publication and eviction behind `Store::acquire`. Defaults such as cache reuse and metadata verification are sometimes described as optional capabilities even though callers cannot compose or omit the underlying stages. Consumers such as Power Eagle need separate source and resolved-file paths with explicit ownership and lifetime.

## What Changes

- Formalize **retrieve** as resolving/fetching source material, with caching completed before its result is returned. Formalize **acquire** as operating on copied results of that source, including Git subpaths. Acquisition update policy can request a fresh retrieval before producing or reusing copied results; provider fetching remains owned by retrieval.
- Add independently callable retrieval and acquisition APIs. Keep the current one-call managed acquisition as an explicitly documented convenience composition.
- Add `cached`, `temporary`, and `no_store` retrieval policies. Cached retrieval retains reusable source material. Both other modes fetch fresh without reading or writing Saucepan discovery, repository or output caches; temporary results have owned lifetimes, while no-store results are delivered to a caller-owned destination.
- Separate retrieval freshness and fallback from acquisition update checks, output reuse, claims and activation. A due acquisition update forces fresh source resolution and compares its selected content. Expose validated refresh configuration rather than only a default serialized TTL.
- Use **revision policy** for whether and how much acquired output history is retained. Reclaim is the execution mechanism, not a competing policy name. Support no retained history and a bounded latest-revision history, while honoring live usage.
- Expose a repository/source-material path and a copied acquired-content path with representation, provenance, verification evidence and lifetime. Git retrieval returns coordinated access to repository data; Git acquisition exports/copies from its exact commit and applies subpath selection.
- Make expected-digest checks and full payload rechecks explicit choices. Keep structural safety checks and the selected persistent metadata contract distinct from optional content verification; managed publication retains the evidence required by its contract.
- Keep app claims and credentials independently opt-in. Claims remain associations with managed outputs; temporary/caller-owned files cannot silently become managed claims.
- Add tests proving absent effects, not merely equivalent final results: no cache reads/writes in fresh modes, no network during copying from an adequate retrieval, no implicit claims, and no path surviving only as a deleted temporary directory.

## Capabilities

### New Capabilities

- `source-retrieval`: independently usable retrieval, cache modes, original material paths, freshness, representations and lifetime.
- `acquisition-refresh`: automatic update checks for acquired selections, with unchanged-subpath reuse and truthful checked-versus-selected provenance.

### Modified Capabilities

- `source-resolution`: provider resolution inside retrieval and configurable cache behavior.
- `output-acquisition`: post-retrieval transformations, explicit output destinations, managed convenience composition.
- `output-policies`: revision policies govern acquired history independently of retrieval caching and acquisition update frequency.
- `resource-access`: distinguish source/output paths and their owning guards.
- `content-verification`: separate expected-content verification, persistent record integrity and structural safety.
- `application-claims`: optional ownership with explicit scope, including policy checks without creating a claim.
- `store-layout`: persistent retrieval records and root-free temporary/caller-owned operations.

## Impact

Refactor `core/source`, `core/store`, their models and CLI dispatch; reuse existing Git, ZIP, hashing, locking and staged filesystem helpers. Add focused `core/retrieval` and `models/retrieval` modules. Current managed lifecycle, query, recovery and shim behavior remain regression requirements. New persistent record kinds may require matching current-format binaries; this establishes no old-reader or pre-reimplementation schema compatibility. No new dependency is presently justified.

This change is confined to Saucepan. Power Eagle's adapter rewrite, binary installer, plugin sandboxing and an IPC lease bridge are separate work. It does not authorize arbitrary mutation of shared Git data or add arbitrary-Git-server archive download support. Existing convenience requests retain their behavior; their legacy `eviction` field is documented as a revision-policy adapter, not a second policy axis. New composable requests use `revision_policy` exclusively.
