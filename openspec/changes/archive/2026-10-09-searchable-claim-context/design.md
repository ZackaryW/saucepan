# Design

## Context

See [proposal.md](proposal.md) for motivation. Inspection found these integration points:

- `models/index/mod.rs::Claim` stores UUID, request, and output UUID inside the application record. `Store::claims` already applies application source filters and joins output availability under the store lock.
- `core/index/claims.rs::claim_record` reconstructs a claim during primitive acquisition. Its existing compare-and-set checks the selected output UUID; `scoped` separately reconstructs the returned claim. Both must change to avoid losing context or returning stale context.
- Managed installation constructs a claim in `core/lifecycle/managed.rs`; managed update/rollback clone application state and change its claim selection, checking the application revision before publication. Context replacement must bump that same revision.
- Index journals already recover application records; context needs no second persistence mechanism. `core/store/access.rs::lease_claim` provides the existing atomic claim-selection/lease boundary. Querying must not imply that lifetime guarantee.
- `cli/mod.rs` accepts JSON files and explicit store roots. `tests/claims.rs` already proves independent claims, output reuse, source filters, and CLI listing.
- `Cargo.lock` locks `globset` 0.4.20. The installed package's `src/lib.rs::Candidate::from_bytes` calls `normalize_path`; `src/pathutil.rs` converts Windows backslashes. Its defaults and grammar serve filesystem globs. This differs from the portable opaque-string context contract.

## Goals / Non-Goals

**Goals:** A small discoverable query API, explicit metadata edits, coherent results, and context preservation across existing workflows. Use the accepted `core/query` ownership boundary.

**Non-Goals:** A query language, persistent search index, project filesystem hierarchy, inferred context, uniqueness/upsert semantics, or atomic query-and-lease. Consumer project identity and path normalization remain the consumer's responsibility. No Powerspec repository or SDK edits in this change.

## Decisions

### 1. Keep metadata on claims, outside acquisition identity

Use a `ClaimContext` alias for `BTreeMap<String, String>` in `models/index`. `Claim.context` defaults to an empty map when omitted and is serialized as a map. This defines optional metadata in the current contract, not a compatibility layer or inherited schema version. Reject non-string values and validate the limits in the application-claims delta.

Do not put context in `models/store::Request`: output provenance and deduplication should remain independent of consumers' labels. Two context-bearing acquisitions can reuse an output while creating distinct claim UUIDs. Context has no uniqueness constraint, and repeated acquisition is not an idempotent lookup/upsert.

Alternative: a separate context index or per-project folders. Rejected because each introduces a second source of truth without a demonstrated lookup-scale need.

### 2. Separate query ownership from index persistence

```text
src/core/
  index/claims.rs       # Claim mutations and shared locked view assembly
  query/
    mod.rs              # Store::query_claims orchestration
    model.rs            # ClaimQuery, Condition, Operator, QueryResult
    matching.rs         # Validate/compile conditions and evaluate context
src/models/index/mod.rs # Persisted ClaimContext and Claim
src/utils/pattern.rs    # Portable string-glob mechanics
```

The query transport models live in `core/query/model.rs`, as agreed for this feature; persistent metadata stays in the existing models tree. Re-export the public query types from `core/query`. No new storage directories and no dependency from index persistence to query evaluation.

Proposed native surface:

```rust
Store::query_claims(&self, query: &ClaimQuery) -> Result<QueryResult>
Store::acquire_for_with_context(
    &self, app: Uuid, request: &Request,
    context: &ClaimContext, options: &AcquireOptions,
) -> Result<Claimed>
Store::set_claim_context(
    &self, app: Uuid, claim: Uuid,
    context: &ClaimContext, expected_revision: u64,
) -> Result<Claim>
```

`acquire_for` remains the empty-context convenience entry point. Context replacement returns the committed claim; callers read a fresh application/query snapshot when they need its current revision.

### 3. Queries are flat, explicit, and deterministic

```json
{
  "app": "00000000-0000-4000-8000-000000000001",
  "where": [
    {"key": "project", "op": "eq", "value": "C:/work/project-a"},
    {"key": "source_alias", "op": "glob", "value": "skills-*"}
  ]
}
```

Return `{ "app": UUID, "revision": NUMBER, "claims": [ClaimView, ...] }`. Use strict deserialization and require `where`; an explicit empty array requests the complete filtered view. Validate every condition and compile patterns once before reading candidates, even for empty stores. All conditions use AND; no reserved context keys or field traversal. Start with a linear scan over one application's claims, ordered by UUID. Context/query limits bound individual input sizes, not total application size.

Acquire the store lock once; read the ready application, apply its source filter, evaluate context, and load each matching output's indexed availability before releasing it. Share internal locked view assembly with `claims` where useful, without calling a lock-taking public API from inside another lock. Return owned values. Do not traverse every application or payload files. Corrupt matched output metadata is an error.

Search gives a snapshot, not a reservation. A caller can lease the returned output UUID for that exact snapshot selection, or use `lease_claim` for the claim's current selection. Either can fail if deletion/removal wins before leasing. Existing recovery on store open/lock remains intact.

### 4. Context edits use existing transaction and concurrency rules

Validate context before acquisition work. Carry initial context through `ClaimTarget` to new claim publication. For primitive updates, merge the context from the current claim under the commit lock after checking its expected selected output; never overwrite it from the stale pre-acquisition snapshot. Return the actual committed claim rather than rebuilding it without metadata. Failed acquisition leaves associations unchanged.

For replacement, validate input, lock, read the ready application, require the expected revision, locate the exact claim, then compare maps. An equal map is a no-op; a different map increments revision and journals the complete application record. No merge-patch semantics. A stale expected revision fails even for an equal map.

Managed installation/bootstrap start with empty context; consumers can subsequently use explicit replacement. This avoids expanding installation request fingerprints for a metadata-only feature. Managed updates and rollback already clone claims, but require regression coverage. Their revision checks must catch intervening context edits. Journals and desired application snapshots naturally carry context; deactivation preserves it, release/uninstall removes it, and output reclaim leaves it present.

### 5. CLI inputs remain JSON files

```text
saucepan --root ROOT acquire request.json --app APP_UUID --context context.json
saucepan --root ROOT set-claim-context APP_UUID CLAIM_UUID context.json --expected-revision N
saucepan --root ROOT query-claims query.json
```

`context.json` contains the map itself, not a query. `--context` without `--app` fails before acquisition. Existing acquisition without context stays valid. The query command's JSON file contains the application exactly once. Results use the usual JSON stdout/error stderr boundary. Mutations accept concrete UUIDs only; neither CLI nor native API treats query results as bulk mutation instructions.

### 6. Utility assessment and planned contract

**Reuse:** Existing serde, maps, locks, and journal code directly. Existing `globset` remains unchanged for release asset matching. Its path normalization and richer filesystem grammar conflict with this feature's opaque-string, Unicode-scalar semantics. A wrapper around `GlobMatcher::is_match` cannot remove candidate normalization. A new regex dependency plus glob translation would add more machinery than the small supported grammar requires.

**Portable helper:** `utils::pattern::StringGlob::compile(pattern: &str) -> Result<StringGlob, PatternError>` and `StringGlob::is_match(&self, value: &str) -> bool`. Neither imports query/claim models nor reads configuration. Another caller could use the same helper to match document labels or inventory tags.

**Contract:** Compile `*`, `?`, literals, and the explicit escapes in the claim-query spec into tokens. Collapse repeated stars. Match the complete sequence of Unicode scalar values, including separators/newlines. Use iterative dynamic programming with polynomial time and linear auxiliary memory, avoiding recursive/exponential backtracking. Report malformed patterns with a character position; application matching adds the condition index/key. Query validation owns input size limits.

**Effects/failures:** Pure in-memory compilation/matching, no filesystem access or caching side effects; repeated calls give identical results. Only compilation can return syntax errors.

**Verification:** Domain-neutral fixtures for empty patterns/strings, zero-length star matches, full anchoring, Unicode scalars versus combining sequences, escapes, literal slashes/backslashes, repeated stars, malformed brackets/braces/escapes, and long nonmatching inputs. Compare bounded small generated cases against an independent simple reference matcher to cover overlapping wildcard behavior. Then test application-owned AND, missing-key, validation, and scoping behavior separately.

No standalone utility package or implementation scaffolding is created during planning. Verify the utility slice before integrating query callers.

## Risks / Trade-offs

- Context strings can label the same project differently (case, slashes, aliases) → preserve strings and document consumer-owned canonicalization; never infer filesystem identity.
- Broad patterns or duplicate contexts produce many matches → deterministic full lists, no implicit first match, uniqueness, or automatic claiming.
- Linear scans and the shared metadata lock limit very large application views → scan one application, compile once, inspect only matching output metadata; defer persistent indexing until measured need.
- Naive claim reconstruction can drop concurrent context edits → preserve context from the locked current claim and verify returned/persisted equality in a controlled race fixture.
- Query discovery can be mistaken for leased access → document exact-output versus current-claim leasing and demonstrate reclaim remains possible after search.
- Users may expect full filesystem glob syntax → document the supported subset and fail unsupported syntax explicitly; test the same string fixtures on Windows and Unix when those runners are available.

## Migration Plan

Implement in verified checkpoints listed in tasks.md. Use temporary roots for integration tests and add standalone example JSON files outside the existing simulated installation. Do not rewrite `mock/` or production stores. The current schema permits omitted context as empty; this change adds no schema-version negotiation, old-store migration, or promise that older binaries can read new fields. Mixed-version writers are unsupported; an executable downgrade must use a compatible store snapshot or a fresh root. Keep active OpenSpec planning artifacts outside implementation checkpoint commits under the existing repository workflow.
