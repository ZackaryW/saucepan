# Design

## Context

See proposal.md for motivation. The initial inspection, before this change, found concrete coupling:

- `core/source/mod.rs::prepare` requires `Store` and `.staging`; `Prepared` exposes only transformed content.
- `core/source/remote.rs::prepare_remote_resolved` resolves release metadata, consults existing outputs, downloads, checks digests and extracts ZIPs in one operation. Original ZIP bytes disappear with preparation.
- `core/source/git.rs::prepare_git` coordinates `source/<uuid>/repo`, resolves a commit, exports and copies a folder. `utils/source/git.rs::export` also resolves submodules/LFS while inventorying; a simple function rename would not make acquisition offline.
- `core/store/mod.rs::publish_prepared` always builds an output manifest, publishes an indexed output and runs reclaim. There is no caller-owned result type.
- `core/claim/ensure.rs` returns a matching available claim before consulting acquisition options. Automatic update checking therefore needs an explicit branch before this shortcut, not another flag ignored on reuse.
- Refresh TTL is serialized but registration always assigns its default; normal callers have no setter. Existing `Eviction::Latest` already implements bounded output history, and ordinary claims are not pins.

The previous Power Eagle audit demonstrated scoped acquisition and full verification of its real `hello-note` files with the current binary. It did not demonstrate a compatible Power Eagle adapter or a lifetime bridge. This proposal does not claim those exist.

## Goals / Non-Goals

**Goals:** independent operations with typed results; explicit effects and ownership; source paths distinct from copied-output paths; automatic output updates when requested; revision history independent of freshness; preservation of existing managed consistency and scoped permissions.

**Non-goals:** dynamic provider plugins, a graph engine, a daemon/scheduler, arbitrary mutation of shared repositories, arbitrary Git-host ZIP endpoints, Power Eagle changes, IPC lease protocols, encryption, schema-version inheritance or pre-reimplementation migration. No compile-time feature matrix is needed to make these runtime operations composable.

## Decisions

### 1. Adopt the user's terminology as the API boundary

```text
source + selector + retrieval policy
       -> retrieve -> source/repository handle and path
                         |
selection + acquisition update policy + revision policy
       -> acquire -------+-> copy/export/extract -> acquired handle and path
             |
             +-- when update is due: call retrieve with fresh resolution first
```

Retrieval operates on the Git repository or original downloaded asset. Acquisition operates on copied results: `folder` is a commit-relative selection for Git and an extraction/copy selection for other material. Two subpaths can consume the same retrieval without two network fetches. A pure materialization function handles copying after policy evaluation; it cannot fetch. The acquisition orchestrator can fetch only by explicitly calling retrieval when its update policy requires it.

The implemented native boundary is `Retriever::retrieve(&RetrieveRequest) -> Result<RetrievedResource>` for inline fresh inputs, `Store::retrieve(&RetrieveRequest)` for managed retrieval, and `Acquirer::acquire(&RetrievedResource, &AcquisitionRequest) -> Result<AcquiredResource>` for offline copying. `Store::acquire_composed` owns automatic update ordering; `Acquirer::refresh` consumes a held source guard before an update. Guard-owned accessors are `source_path()` / `path()`. `RegisteredRetriever` provides metadata-only fresh access, with scoped session variants enforcing application policy. New JSON DTOs distinguish inline source definitions from registered source UUIDs. They never infer application identity from a cache path.

The existing `Store::acquire` and CLI `acquire FILE` remain convenience compositions with existing result shape/defaults. New CLI commands are `retrieve FILE`, `acquire-retrieved FILE`, `source-path RETRIEVAL_UUID`, `run-retrieved FILE -- PROGRAM ARG...`, and `run-acquired FILE -- PROGRAM ARG...`. `acquire-retrieved` takes a managed retrieval UUID or an explicit caller-owned material descriptor. It never treats an arbitrary JSON path as a trusted managed record. `run-acquired` composes retrieval and acquisition with a child consumer, allowing temporary copied results to have an explicit lifetime.

Alternative rejected: extending `AcquireOptions` with many booleans while keeping publication mandatory. That changes configuration, not composability.

### 2. Retrieval policy decides cache effects before any I/O

| Mode | Source material | Shared cache | Ownership |
| --- | --- | --- | --- |
| `cached` (managed convenience default) | Fetch if required; reuse verified matching material | Can read/write discovery, Git dependencies and retrieval records | Saucepan; native handle holds a source lease |
| `temporary` | Fresh origin resolution and material for every independent call | No discovery, Git/object/LFS, retrieved-material or output cache reads/writes | Guard-owned scratch; cleanup after last dependent handle |
| `no_store` | Fresh origin resolution and material for every independent call | Same bypass as temporary | Explicit absent caller destination; no managed record/history |

Both fresh modes require real origin work even for an exact commit that happens to exist in shared Git objects. Do not use alternates, shared clones or conditional HTTP responses that require cached payload bytes. Local sources are inspected and copied fresh; "fetch" does not invent network access for local input. Temporary disk staging is permitted and reported; no-store does not mean diskless.

Inline temporary/no-store calls do not initialize, open or recover the default store and do not require a source registration, app, keyring or home directory. Registered source/app requests explicitly require the owning root for metadata/policy reads; they still bypass cache payloads and cache mutation. The ordinary store-open recovery path must not accidentally run on that read-only metadata path: pending committed recovery returns an explicit error requiring separate reconciliation before a no-store app operation.

For cached retrieval, support `freshness: {mode: "max_age", seconds: 86400}` (default), `always`, and `cache_only`. Cache-only requires a usable material hit and cannot hydrate missing Git dependencies or download missing bytes. Stale fallback is separately explicit and cached-only. Invalid combinations, including fresh mode plus cache-only/stale fallback, fail before network or destination creation. Expose source-default configuration with a guarded native setter and `configure-source SOURCE_UUID FILE`; use the provider-lock then mutation-lock order already used by visibility changes. Per-call overrides do not mutate those defaults.

### 3. Git source access is repository access with a pinned commit

The existing provider repository is bare. Return it as `kind: git_repository`, with exact commit and dependency inventory, not as a pretend filesystem checkout. A cached source handle holds coordinated read access so Saucepan fetch/maintenance cannot mutate the repository during use. Consumers may perform read-only Git inspection. They must not edit refs/config, fetch, prune, or use this path as the acquired plugin directory. Bare `source-path` lookup is only an address; `run-retrieved` or the native guard retains access.

Split Git fetching/hydration from export. Retrieval makes the pinned commit, required submodule commits and LFS objects available under the selected cache mode; acquisition exports using only these supplied inputs. Resolve links and executable metadata using existing behavior. The first slice can hydrate the complete commit rather than optimize subpath-dependent hydration; subpath remains an acquisition concern. Acquisition copies cannot use hardlinks or symlink aliases into mutable source data.

For HTTP/release ZIPs, `source_path` addresses the original archive file. Acquisition extracts it to another directory. Raw payload acquisition copies it without reinterpretation. For local directories, retrieval returns a captured directory copy so concurrent editing of the original does not silently alter an already returned retrieval. No automatic GitHub archive transport is implied: callers can use a URL source for an explicit archive URL, while Git transport preserves exact commit/submodule/LFS semantics.

### 4. Acquisition update policy is separate from retrieval freshness

Use `update_policy: {mode: "manual"}` by default, `always`, or `{mode: "max_age", seconds: N}`. Evaluate it before managed-output or claim reuse. It controls when the selected output is checked, not how many revisions are retained. Checks are demand-driven, not background tasks.

When due, a moving selector must be freshly resolved by retrieval even if its normal source TTL is still fresh. A retrieval freshly performed for this same operation can satisfy the check; do not fetch twice. An explicit `cache_only` retrieval constraint plus a due update requiring freshness fails preflight rather than silently weakening either choice. Exact commit selectors never switch to another commit. Stale fallback preserves the previous acquired selection, reports stale/pending update, and does not advance the successful acquisition-check timestamp.

Compare the resulting selected manifest (bytes, paths, executable bits and relevant dependency evidence), not just the repository head. If only files outside the requested subpath changed, reuse the existing compatible output and retain its original provenance. Return `checked_source` separately from `selected_source` and `changed: false`; never relabel old bytes as a new commit. Changed selected content creates a new immutable output. Explicit `new_entry` remains an independent request to allocate a revision even for unchanged content.

Persist check state separately from immutable outputs. A scoped acquisition uses its claim identity; an unclaimed managed acquisition uses a canonical source/selector/selection plus effective update/revision/verification-policy key. Different scopes do not share selection/check state merely because they share content. Use a revision-checked acquisition-state record with selected output, last successful check and observed source identity. Policy changes cannot reuse a looser freshness proof. Failed checks/publication leave prior selection and successful check time unchanged. Serialize same-key refreshes with a bounded operation lock, then publish state/claim changes through the existing journal; recheck app authorization and revision before commit.

Automatic claim updates require the update action and permitted policy. They preserve context and reuse the same claim UUID. They never implicitly activate an executable or change an existing binding. Existing managed installation/update continues to own probe and binding publication. A changed candidate that has not passed its required checks cannot replace the prior selection.

### 5. Revision policy decides acquired history

New composable requests use `revision_policy: {history: "none"}` or `{history: "latest", keep: N}` where N is positive and includes the newest revision. Managed default is latest ten. No history normalizes to retaining the newest revision only, not overwriting its predecessor's bytes. New publication can temporarily coexist with an older leased/protected revision; reclaim removes the old content only after all existing protections end.

Existing `policies.eviction.strategy=latest, keep=N` is translated at the convenience boundary into the same revision policy; keep=1 means no historical window. There is one effective evaluator. Do not add two simultaneously applicable retention knobs. New requests reject an `eviction` field and mixed spellings rather than guessing precedence. Internal persisted counts can remain readable without a mass rewrite; user-facing docs and new APIs use revision policy. This is current-API adaptation, not a legacy-store decoder.

Keep the existing maximum surviving demand rule: a no-history claimant cannot delete revisions another current output/claim policy retains. Claims remain associations rather than pins. `reclaim_policy` still means how unavailable claim content is recovered, and `useable_app` still means active-binding protection. Neither is renamed to revision policy.

Acquisition destinations are `managed`, `temporary`, or `directory` (caller-owned, create-new). Managed output can only consume cached retrieval in this change. Temporary/no-store retrieval cannot be silently promoted to managed output or a persistent claim. Cached retrieval may feed a temporary/caller-owned output. Fresh source and output destinations must be disjoint. Caller destinations are never overwritten or later reclaimed by Saucepan; repeated updates deliver to fresh destinations chosen by the caller. Temporary/caller-owned output rejects explicit managed revision policies. Their caller owns any history outside Saucepan.

### 6. Verification reports what actually happened

Keep four distinct concepts: structural validation; expected-digest verification of retrieved bytes; managed manifest identity/metadata integrity; optional access-time full recheck.

Retrieval exposes `verification: none | if_available | required`, defaulting to if_available. An explicit expected digest is a constraint, so `none` plus a digest is invalid. Required fails if no supported expected digest is available; it never reports a newly computed self-hash as upstream authentication. Git object/dependency validation remains part of obtaining a coherent repository and is reported separately. A later stronger request must check cached bytes or fail; weaker verification evidence cannot satisfy it.

Temporary/no-store operations with none need not produce a whole-payload digest unless another explicitly chosen operation requires one. Managed publication still requires a complete manifest for immutable identity and reuse; hashing for storage identity is not presented as optional upstream verification. Full access checks remain opt-in and store basic/keyring integrity remains independent. No option disables containment, extraction limits, collision handling or configured app permissions. Document these always-on invariants explicitly rather than calling them optional features.

### 7. Paths carry ownership; strings do not extend lifetimes

Native results carry non-serializable guards and structured descriptors (kind, path, source provenance, verification evidence, optional persistent identity). An acquired temporary handle can retain its retrieval parent as needed. Source refresh and source cleanup coordinate with source leases; output reclaim coordinates with output leases. The locks are distinct. Do not hold a source lease while attempting to acquire its mutation lock for an automatic refresh: release/retrieve/rebind before materialization, keeping any previous output lease intact.

CLI `retrieve` emits a receipt containing `request` and `descriptor` with normalized source identity; cached ownership contains the persistent retrieval UUID. `acquire-retrieved` accepts `retrieval: {mode: managed, uuid}` or `retrieval: {mode: caller, request, descriptor}` alongside `acquisition`. Caller reopening rechecks files and discards serialized verification/check evidence; supplied provenance and expected digests remain caller assertions. `run-acquired` instead accepts a retrieval request and acquisition request. For cached addresses, `source-path` does not retain a lease. No-store emits the completed caller-owned destination. Temporary CLI operations require `run-retrieved ... -- PROGRAM`, `run-acquired ... -- PROGRAM`, or same-process composition; reject a bare temporary JSON path that would be deleted at CLI exit. Source consumers receive `SAUCEPAN_SOURCE_PATH` and representation/commit metadata; acquired-output consumers receive `SAUCEPAN_RESOURCE_DIR`. Reuse the existing native inherited-handle model, including abrupt wrapper exit; do not introduce PID-only liveness. Crash leftovers are eligible for cleanup only after native liveness checks, never a TTL guess. Temporary owned directories use a reserved marker and a temporary-parent coordination lock; later consumer operations sweep only closed markers. Cached source refresh/purge checks inherited source markers after taking source coordination. This protects cooperating consumers; children must preserve inherited handles.

### 8. Package and persistence responsibilities

| Area | Responsibility |
| --- | --- |
| `models/retrieval/` | Source inputs, retrieval/cache/verification policies, material descriptors |
| `core/retrieval/` | Cache policy, source leases, provider orchestration and retrieval publication |
| `core/source/` | Git/HTTP/local provider mechanics without implicit output publication |
| `models/store/`, `models/policy/` | Acquisition destination, update/revision policies, output results |
| `core/store/` | Materialization, managed output publication, selected-content comparison and revision enforcement |
| `core/claim/` and `models/claim/` | Scoped authorization, allowed policy bounds, claim check/selection state |
| `core/index/` | Typed retrieval/acquisition-state records and recoverable membership |
| `cli/` | Separate retrieval/composition commands, receipt parsing, root selection and structured errors |
| `core/consumer.rs` | Child environments and transfer of source/output/temporary lifetimes |
| `utils/` | Domain-neutral bounded transfer/copy/extract, filesystem publication and lifetime mechanics |

Cached raw material lives under `source/<source-uuid>/retrieval/<retrieval-uuid>/`; Git repository/dependency caches keep provider-owned locations under `source/<source-uuid>/`. Retrieval descriptors and acquisition-state metadata are typed `index/<uuid>/index.json` records, distinct from apps/outputs. Root catalog membership, basic/keyring protection and journals apply to these new records. Acquisition outputs remain `store/<entry-uuid>/`. Reclaim of output revisions does not delete retrieval material. Add explicit trusted `purge-retrieval RETRIEVAL_UUID` to retire a cached retrieval with live-use deferral; this initial operation does not prune shared Git objects or delete source registrations. Do not invent a source-cache latest-ten default from output revision settings.

Inline cached retrieval registers/deduplicates its source through trusted administration; app-scoped calls use an existing source identity and existing permissions. No app is synthesized. An app may choose not to claim, but its identity is still checked; if configured scope restrictions require an explicit scope, omitting the authorization scope is denied rather than falling back to administration. Scope context without a claim is not a registration or ownership side effect.

### 9. Utility plan

No dependency changes are planned. Existing `tempfile`, `ureq`, `zip`, SHA-256, native locks and `process-wrap` already supply the mechanics.

- **Bounded transfer:** extract the HTTP body-to-writer loop into a helper accepting `Read`, `Write`, byte limit and optional digest calculation. Reuse `tree::digest`/existing SHA implementation. Return byte count and optional digest, never provider/cache policy. Useful to an updater outside Saucepan. Verify exact limit, limit+1, short/error reads/writes, digest vectors and no destination publication after failure.
- **Git hydration versus export:** retain the existing `Git` helper, split network hydration from an offline export using explicit repository/dependency paths and exact commit. Accept a caller-owned scratch root; never derive Saucepan paths. Useful to a build artifact exporter. Verify branch advance, pinned commits, submodule/LFS completeness and zero fetch invocations during offline export; preserve path/link limits.
- **Staged directories and copying:** reuse `fs::staged_directory`, `tree::copy_tree` and `archive::extract_zip`. Their comments require caller serialization and a trusted parent; add cross-process destination coordination for create-new delivery, not an unsupported claim of universal atomic no-replace rename. Verify occupied/racing destinations, rollback, limits and contained files. ZIP extraction already takes `Read + Seek`, destination and selection; it needs no new wrapper.
- **Native lifetimes:** reuse `utils/lock`, existing marker/launch mechanics and `tempfile` ownership. Application-level retrieval and output guards remain in core. Verify wrapper death, dependency retention, failed launch cleanup and source-refresh versus reader coordination on each executed host. Do not implement a new lease service or declare host verification from compilation alone.

Implement and verify any extracted helpers in a coherent checkpoint before integrating provider orchestration; avoid generic policy abstractions in utils.

## Risks / Trade-offs

- Cached ZIPs and acquired trees duplicate bytes -> expose separate identities and explicit retrieval purge; do not couple source deletion to output history.
- Source path consumers may misuse a bare repository as a checkout -> report kind/commit, document read-only Git access, test copies separately. OS permissions remain the filesystem boundary.
- Automatic refresh can cause duplicate network work or stale claim writes -> per-intent coordination, explicit successful-check state and existing revision fences.
- Selected subpath unchanged while repository advances -> preserve old output provenance and return checked-source evidence separately; test unrelated commits and changing dependency content.
- Fresh modes may be slower -> that cost is the explicit no-cache choice; never secretly consult shared caches.
- Holding source leases can delay refresh -> bounded lock waits and no lock upgrade while a handle is held.
- CLI strings cannot own temporary files -> child-process or caller-owned destination contracts, not time-based grace periods.
- Larger public API -> strict request types and a documented composition matrix; reject unsupported combinations before effects.

## Migration Plan

Implement dependency-ordered checkpoints: contracts/helpers, retrieval, post-retrieval acquisition, automatic update/revision policies, optional claims/CLI, acceptance/docs. Preserve the existing managed entry points as adapters with their existing defaults; new DTOs use the formal terms. Do not rewrite existing records merely to rename a policy. New record kinds/fields are current-contract additions and may be rejected by older binaries; do not promise reverse readability or restore earlier schema versions. Use fresh disposable roots for tests, leave historical mock and user stores untouched, and ship matching manager/shim builds. Roll back with a separately preserved root when an older binary cannot read new records; no automatic reverse conversion is provided.
