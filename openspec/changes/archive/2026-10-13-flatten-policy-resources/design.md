# Design

## Context

See `proposal.md` for motivation. Inspection found the following coupling:

- `models/claim/policy.rs` bundles `AccessPolicy`, `Registration.defaults`, permitted overrides and `Overrides`; `core/claim/registry.rs` validates a claim by recomputing these embedded values.
- `models/retrieval` embeds cache/freshness and payload verification. `models/store/composition.rs` embeds update, identical, revision and protection choices. `models/source` duplicates the older refresh TTL and newer retrieval freshness.
- Release filters and selection strategy are embedded in asset selections. `core/policy` already exists and owns output-policy behavior; it is the appropriate home for shared resolution.
- Metadata catalogs, record envelopes and journals already coordinate immutable provenance with mutable application revisions. `core/store/updates.rs` includes effective policy choices in update identity; claim ensure coordinates scope, context and publication.

The implementation of `compose-retrieval-and-acquisition` is the baseline, including its unpublished delta specs. Its tests cover root-free fresh operations, separate path lifetimes, offline Git copies, scoped updates and independent history. Those guarantees must survive this shape correction.

## Goals / Non-Goals

**Goals:** one definition per policy UUID, explicit associations, predictable evaluation and inspectable provenance; preserve easy default operations and independent opt-ins; integrate into existing module responsibilities.

**Non-Goals:** a general IAM language, transitive policy inheritance, a graph database, remote authorization service, filesystem isolation, encryption, automatic store migration, or Power Eagle adapter implementation. A flat policy payload can contain structured values belonging to its own type; flatness means independent identity and references, not banning JSON arrays or structured provider input.

## Decisions

### 1. Immutable definitions and small resource associations

Persist a typed record at `policies/<uuid>.json`, protected by the root's existing integrity mode and catalog. Its payload has `uuid`, `type` and fields specific to that type. Applications, sources and claims expose `policy_refs: [UUID, ...]`; requests select behavior with the same reference shape. Resource fields such as an explicit scope, Git ref, subpath, expected content digest and destination path remain on the resource/request.

User decision: a changed definition gets a new UUID and explicit adoption. Registering an existing UUID with equivalent typed content is a no-op; different content is `policy_conflict`. There is no edit-in-place API. Policy records contain no policy references, parent UUIDs or embedded policy catalogs. Resolution is a bounded lookup, not a recursive graph walk.

Rejected alternatives: mutable shared records would change unrelated consumers; retaining inline bundles alongside references would create two authorities; encoding relationship semantics into UUID text would conflate identity with policy.

### 2. Explicit type coverage without a universal settings bag

Use a tagged Rust enum in `models/policy`, with small modules for definitions, references and evaluated results. The initial type inventory covers existing behavior:

| Type | Owned settings / applicability |
| --- | --- |
| `access_policy` | Optional source/action/scope allow sets; registered authority attachments, never a request-supplied grant |
| `source_visibility_policy` | Public discovery boolean; source attachment only |
| `claim_visibility_policy` | Public scopes; application attachment only; credentials still independently govern public claim discovery |
| `cache_policy` | `cached`, `temporary`, `no_store`; retrieval behavior, with destination path supplied separately |
| `refresh_policy` | `max_age`, `always`, `cache_only`; retrieval freshness |
| `stale_policy` | Explicit stale fallback; cached retrieval only |
| `verification_policy` | Payload expected-digest behavior and independently selected path recheck behavior, distinguished by an explicit `policy_target` |
| `github_release_asset_filter` | Platform constraint and alternative asset patterns |
| `selection_policy` | `policy_target: github_release_asset_filter`, `strategy: first_match_win` |
| `identical_pull_policy` | Reuse or new output entry |
| `update_policy` | Manual, always or max-age copied-result update |
| `revision_policy` | No historical window or latest positive count |
| `reclaim_policy` | Report unavailable, restore exact or refresh unavailable content |
| `useable_app_policy` | Active-binding protection |
| `policy_constraints` | Existing allowed identical/reclaim choices, retention bounds, protection requirement/permission and update-frequency bounds |

Constraints reference behavior **types and permitted values**, not policy UUIDs. This permits new policy IDs within an existing allowance without changing authority. Unknown types, targets and fields are rejected. Policies with incompatible stages fail before effects rather than being silently ignored. App/source attachments can supply defaults for applicable stages; the evaluator projects those attachments for the current operation. A direct request reference must apply to its operation or declared composed stages.

Release selections have an explicit ordered `filter_refs` list. Filters themselves remain leaf records in the same catalog; `selection_policy` names the filter type, not other records. Other policy lists have no priority-by-array-order. Duplicate IDs are invalid. Distinct singleton definitions at one precedence level conflict even if their values happen to agree; different IDs with equivalent effective values can still permit claim/content reuse.

Credentials and store integrity remain separate registration/bootstrap contracts. Resolving policy cannot be required to discover the key used to verify that policy. No signing key, token or credential verifier belongs in reusable policy definitions.

### 3. Resolve authority separately from behavior

`core/policy` takes a verified catalog snapshot, attachment revisions, caller/action/resource context and operation selections, then returns a typed evaluated plan and explanation. It performs no fetching, copying or implicit registration.

Authority uses all applicable registered access restrictions and constraints. Configured allow sets intersect; omitted dimensions impose no extra restriction, while empty sets permit nothing. Behavior selection or a caller-supplied definition cannot replace those restrictions. Keep the existing explicit trusted-local-administration boundary and application ownership checks; selecting a policy UUID is not authentication or a grant.

For a new operation, select each behavior axis in this order: explicit request, application default, source default, built-in default. Existing claims instead use the behavior references/effective built-in choices captured when created or explicitly adopted; a later default change must not reinterpret them. Evaluate the selected behavior against every applicable registered constraint. Required usable-app protection cannot be removed by omission or a different default.

Defaults stay cheap: no policy files are synthesized merely because a caller omitted a choice. Retain cached retrieval with 24-hour freshness and no stale fallback, manual acquisition updates, reuse, latest-ten managed revision history, report-unavailable recovery, optional payload verification as currently defined, metadata-only path access, private discovery and credential-free app registration. Temporary/no-store imply fresh retrieval; explicit cache-only or stale-cache choices conflict. Destination data is not stored inside reusable cache policies. Explicit managed-only revision/protection policies conflict with temporary or caller-owned acquisition.

Due acquisition updates still force a fresh retrieval and compare selected content. An incompatible cache-only choice fails rather than silently defeating the update. Revision policies change retained output history, not update frequency or source caching. The existing maximum surviving retention demand and lease/binding protection evaluator remains authoritative for reclaim.

### 4. Portable catalogs keep operations truly optional

Native callers can supply a `PolicyCatalog` containing standalone definitions. CLI fresh/composed operations accept `--policies FILE` containing the equivalent array; request `policy_refs` and `filter_refs` select definitions from it. This is a sidecar catalog, not policy bodies nested under app/claim fields. No catalog is needed when using defaults. Retained path-verification convenience flags compile to ephemeral definitions and references through the same resolver; conflicting flags and explicit references fail instead of gaining hidden precedence. Remove the former global force-refresh/allow-stale flags and native option fields: refresh and stale choices use typed policy references. Native path-mode conveniences follow the same ephemeral-definition route.

Inline fresh operations resolve only the supplied catalog and built-ins, without home resolution, metadata recovery, keyring access or persistent writes. Registered-source fresh operations may read the verified metadata and registered constraints but retain their existing metadata-only/no-cache-effect path. A local definition cannot shadow a persisted UUID. Reject duplicate IDs within a supplied catalog, including identical duplicates, and collisions with persistent IDs; callers reference the persisted definition directly instead.

Request catalogs can select behavior within allowances, never supply authoritative access/visibility/constraint attachments. Persisted app/source/claim references must resolve to registered definitions. A managed operation using ephemeral behavior definitions must explicitly register them before publishing durable references; it fails preflight if this prerequisite is missing rather than auto-registering arbitrary caller records. This keeps fresh use independent and durable state self-contained.

### 5. Explicit adoption with operation fences

Exact inspection is a first-class native operation and CLI command: `saucepan policy POLICY_UUID` (optional `--root`, no query file). Return the verified typed definition itself, not its file path or merged effective settings. Reuse `invalid_request` for malformed/unknown caller IDs and `integrity_error` for missing/corrupt expected catalog records. Trusted local inspection does not require an app UUID; explicit app sessions must authenticate as configured and have an authorized readable association to the definition, with no fallback to administration. A caller-owned catalog supports the same exact lookup in memory without store setup. Lookup itself performs no provider work or adoption; normal store-open recovery remains separate.

Provide native and CLI policy registration/inspection and reference-replacement operations. Application and claim adoption use the expected application revision; source adoption uses its own monotonically increasing attachment revision. Application/source authority changes remain trusted administration with applicable proof. An authorized claim update may adopt permitted behavior within unchanged app constraints. Replacements are atomic and validate existing claims; if tightening constraints would invalidate any survivor, fail without partial adoption. A caller can first explicitly adjust affected claims and then tighten constraints.

Resolve once before side effects, capture policy UUID/content digests and relevant app/source attachment revisions, then recheck those fences before publication. An in-flight operation cannot commit with superseded authority. Unrelated catalog additions should not invalidate an operation merely because the global generation advanced.

Effective values and their provenance are derived output evidence: each axis identifies the policy UUID/digest or built-in default used. Such snapshots are not an independently editable source of authority. Current authorization is evaluated even when replaying a saved output request or a previously successful check. Historical snapshots retain revision demand and reproduction evidence without granting present permission.

Claim identity remains normalized requested content plus explicit app/scope, not policy UUIDs. Exact and covered requests compare effective compatibility. Reuse with equivalent definitions does not silently replace the existing claim's references. Incompatible choices fail with the existing conflict/overlap behavior; explicit adoption changes references on the same claim and preserves its context, selection and binding. Changed update behavior invalidates the relevant successful-check evidence conservatively; unrelated/equivalent changes need not cause a network fetch. Preserve original historical output evidence.

For this change, registered definitions are retained indefinitely; there is no deletion API. That avoids dangling historical references, pending-journal races and the need for a second policy garbage collector. Removing a resource attachment is supported, but removing a definition is separate future work. Definition UUIDs cannot be reused.

### 6. Fit the existing package and publication machinery

| Area | Responsibility |
| --- | --- |
| `models/policy/{record,references,effective}.rs` | Tagged records, strict validation, reference lists, derived explanations |
| `core/policy/{catalog,resolve,adopt}.rs` | Lookup/registration, pure evaluation, explicit revision-checked associations; keep existing retention code here |
| `core/claim/{security,persistence,registry,policy}.rs` | New policy record kind/path, journal membership and loading; delegate evaluation rather than retaining a second merger |
| `core/retrieval`, `core/source`, `core/store` | Execute evaluated plans; preserve provider, update, reclaim and lifetime algorithms |
| `core/query`, lifecycle/shims, CLI | Use the same resolved authority and projection; expose derived visibility/evidence without bypasses |

Utility assessment: reuse existing UUID parsing, hashing, locking, bounded JSON, atomic publication and test fixture helpers. The policy resolver is domain logic, not a generic utility framework. No general policy library, graph abstraction, repository reorganization or parallel persistence mechanism is warranted. Public inputs change together in the integration checkpoint; temporary internal adapters can keep earlier checkpoints buildable but must not become a supported second public contract.

The subsequent user-requested crate review is recorded in `docs/composition-utilities.md#crate-reuse-decisions`, with inclusion/defer reasons and reconsideration criteria. The implemented cleanup consolidates on the existing `sha2` 0.11 dependency and makes the already locked `hex` crate a direct dependency for digest encoding. `walkdir` and `dunce` remain candidates pending equivalent filesystem/Git behavior checks; neither was added. This does not change policy semantics or reduce the required public-contract cutover.

### 7. Concrete Power Eagle shape

The following policy bodies illustrate the planned contract; an application stores references to defaults and constraints, and a claim stores adopted behavior references. IDs below are illustrative. These are not commands accepted by the current binary.

```json
[
  {"uuid":"10000000-0000-4000-8000-000000000001","type":"access_policy","scopes":["library:main"]},
  {"uuid":"10000000-0000-4000-8000-000000000002","type":"update_policy","mode":"max_age","seconds":86400},
  {"uuid":"10000000-0000-4000-8000-000000000003","type":"revision_policy","history":"latest","keep":3},
  {"uuid":"10000000-0000-4000-8000-000000000004","type":"reclaim_policy","strategy":"restore_exact"}
]
```

Register those definitions, register Power Eagle with its stable registration key and policy references, and register the Git source. Ensure its `examples/hello-note` selection under scope `library:main`; the claim adopts the resolved behavior references and selects a managed copied output. Acquisition returns guarded copied files; retrieval returns guarded repository material. A running Eagle webview still needs an owned copy or a future IPC lease integration before dynamically importing managed files. Flat policies do not solve that separate lifetime boundary.

To change history from three to five, register a new revision-policy UUID and explicitly replace the relevant app default and/or claim association at its expected revision. Other apps and claims keep their existing policy meanings. No app UUID is needed for ordinary anonymous source discovery or unclaimed fresh operations.

## Risks / Trade-offs

- More record references and metadata reads → resolve one coordinated snapshot, reuse immutable definitions within an operation, avoid scans or graph traversal.
- Confusion between defaults and authority → typed applicability, fixed precedence, intersected constraints and tests showing request policies cannot grant access.
- Policy adoption during a probe/fetch → capture narrow attachment revisions and verify again at commit; preserve old selection on conflict.
- Extra setup for custom managed policies → allow batch registration and inspectable references; defaults and anonymous queries remain file-free, fresh catalogs remain ephemeral.
- New public/store contract breaks earlier nested consumers → explicit refusal and fresh-root examples; no silent mutation of existing installations.

## Migration Plan

Use a new named flat-policy catalog discriminator, not the old project's schema/version inheritance. Earlier nested-policy stores, including the immediately preceding composition layout, return `unsupported_store` before recovery, key creation or mutation. Current basic/keyring envelope algorithms remain; supported flat stores preserve their selected mode and fail closed on custody failure. Mixed flat/inline recognized records fail validation.

Implement and validate in fresh isolated roots, preserving historical `mock/` and user stores. Update all live README/docs examples and test builders to references, remove public nested override/eviction compatibility adapters, and clearly label prior verification documents as historical when their schemas differ. No automatic converter is part of this change. Rollback uses the matching previous binary with its separate previous-format root; old binaries must reject new roots.

Before this change is synchronized, synchronize/archive `compose-retrieval-and-acquisition` first, then apply these deltas. Review the resulting specs for its old inline settings references, especially acquisition-refresh and source-retrieval: their behavioral terms remain, while this change defines canonical policy configuration through references. Do not rewrite archived history or auto-archive as part of planning/apply.
