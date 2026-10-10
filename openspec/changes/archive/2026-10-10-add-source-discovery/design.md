# Design

## Context

`SourceRecord` has identity, provider, refresh policy and cached discovery but no visibility. `register_source` already deduplicates normalized definitions through the signed catalogue. `core/query` only searches claims. App sessions carry credentials; the shared evaluator currently assumes scope-bearing claim resources. See proposal.md for motivation. The implemented active claim-registry change is the code baseline; synchronized claim specs are older and are not rewritten in this change.

## Goals / Non-Goals

**Goals:** A small metadata catalogue query, usable by a credential-free unrestricted app before claiming, with explicit anonymous visibility.

**Non-Goals:** Query DSLs, regex/globs, source labels, new ownership scopes, source acquisition authorization changes, anonymous claims, provider refresh, graph indexes, encryption changes, migrations, other repositories, mock refresh, sync/archive.

## Decisions

1. Add `public_discovery: bool` to authenticated source records, default false. Use a trusted `set-source-visibility UUID public|private` command rather than overloading provider definitions or claim policies. Registration preserves existing flags; app sessions cannot mutate them. Omitted flags on already authenticated records mean private, with no format version or unsigned fallback.
2. Put query transport types in `core/query/model.rs`, source query execution in `core/query/source.rs`, and expose `AppSession::query_sources`. `Store::query_sources` without a session is explicitly anonymous, never implicitly administrative. Return `{generation, sources:[{uuid, source, public_discovery}]}` and exclude potentially large cached asset data.
3. Query filters are exact optional fields, bounded strings (1..4096 UTF-8 bytes), no claim context map. Stored `github_repo` registration normalizes to a Git origin; consumers use `type: git` and its stored origin. Contradictory valid filters return no matches. Do not normalize query strings or fetch for matching.
4. Use one lock and authenticated catalogue snapshot; sort views by UUID. Read/authenticate every catalogue source, including hidden candidates, so tampering cannot silently change catalogue results. App identity/proof/discover action validated before returning even an empty catalogue. Intersect source allowances and existing application view filters. Reuse the pure policy evaluator through a source-specific projection that removes the inapplicable claim-scope dimension; do not weaken claim authorization or reinterpret an omitted claim scope globally.
5. Reuse serde, UUID, catalogue persistence, locking and policy helpers. No new portable utility or dependency is required; this is domain wiring. Verify core behavior as one checkpoint, then CLI/consumer flow as a dependent checkpoint.

## Risks / Trade-offs

- Public source definitions may contain local paths or URLs with sensitive text → private default, explicit trusted publication, documented full-definition exposure.
- Linear catalogue scanning → appropriate for the current filesystem catalogue; no speculative indexing subsystem.
- Search then ensure is not atomic → ensure rechecks existing acquisition policies, preserving the existing contract.
- Already built older binaries may reject the new field → no backward-reader promise; this project has no legacy migration requirement.

## Migration Plan

Deploy the rebuilt library/CLI. Existing signed records with no visibility flag remain private. Test with disposable roots, leaving historical mock content untouched. Do not run older writers against newly written metadata. No automated data migration or cross-repository consumer change is planned.
