# Design

## Context

See `proposal.md` for motivation and scope. Today `Application.claims` in `src/models/index/mod.rs` owns all claims. `core/index/claims.rs` allocates a new UUID on every acquisition. `core/index/mod.rs` journals whole application records as plaintext, and `core/lifecycle.rs` persists a desired application snapshot in operation receipts. Reclaim, bootstrap, managed update, query, and claim leasing read that embedded map directly. Moving just the live claim files would therefore leave both correctness gaps and plaintext copies.

The existing native `ResourceLease` in `core/store/access.rs` already owns the entry coordination lock. Its process-inheritance path is deliberately separate. The package remains Rust-only. OS credential storage uses native Rust keyring packages, and resource lifetime ownership stays in the Rust library and dedicated CLI execution boundary.

Confirmed choices: readable JSON is HMAC-protected rather than encrypted; credentials are opt-in per application and independent of source/claim restrictions; explicit scopes have independent claims even when their outputs are shared. The package uses `models/`, so the requested model folder is `src/models/claim/`.

## Goals / Non-Goals

**Goals:** Make one claim authority serve acquire, query, access, policies, and lifecycle; preserve atomic publication and native lifetimes; eliminate duplicate or redundant ownership within a scope; keep source/output identity independent of application labels and credentials.

**Non-Goals:** Old-schema migration, arbitrary glob-based ownership, cross-application claim merging, OS-user isolation, metadata or payload encryption, rollback-proof storage against complete filesystem snapshots, language bindings, a Python SDK, and a Powerspec repository port. Context glob matching remains read-only query behavior.

## Decisions

### 1. Separate storage and package ownership

```text
.saucepan/
  root.json                         # store identity, contract discriminator and authenticated catalog
  source/<source-uuid>.json
  source/<source-uuid>/              # provider working data when needed
  index/<app-uuid>/index.json        # app registration, revision, binding, shim ownership
  index/<output-uuid>/index.json     # output provenance and availability
  claims/<app-uuid>/index.json       # HMAC-protected registry and registered policies
  claims/<app-uuid>/<claim-uuid>.json # readable claim bodies with HMAC tags
  store/<output-uuid>/               # acquired content
  bin/
  .staging/                         # authenticated recoverable transactions
```

The JSON record payloads remain readable. An envelope carries a record kind, store/app/member identity as applicable, generation, payload, and HMAC tag. Authenticate the exact stored UTF-8 payload bytes together with length-delimited header fields; capture the payload as raw JSON rather than reparsing and reserializing it for verification. Reformatting signed payload bytes invalidates the tag. Enforce bounded envelope parsing, reject duplicate/unknown fields, then verify before interpreting the payload as trusted application state. The tag itself is excluded from the signed input.

A key-backed root catalog records expected live app/source/output identities and digests; each app registry records membership and claim-envelope digests. Verify the catalog and referenced records together so deleting an app or member cannot be interpreted as an empty registry. Source definitions and output provenance used in policy decisions must also be authenticated; otherwise editing a source URL could bypass a source-UUID restriction. All catalog, record and availability updates join the existing coordinated transaction. This detects missing, modified or transplanted metadata, but not replacement of a whole store by an earlier internally valid snapshot. A contract discriminator does not revive old schemas or compatibility decoders.

| Location | Responsibility |
| --- | --- |
| `models/claim/{mod,policy,security}.rs` | Claim, explicit scope, coverage view, registry, policy registrations, credential request/result contracts |
| `core/claim/{mod,registry,matching,policy,security,recovery}.rs` | Authorized app session, ensure/reuse, persistence, policy validation, unavailable-content handling |
| `core/index/` | App/output records and coordination of multi-record publication; no embedded claim authority |
| `core/query/` | Validate queries and match an authorized coordinated claim snapshot |
| `utils/crypto.rs` | Domain-neutral randomness, derivation, HMAC generation and verification mechanics |

Move the existing claim types/operations and their tests; do not leave a second mutable copy in `index`. Keep general output policies in `models/policy`; registered application policy contracts reference those types. Alternatives considered: extending the embedded map preserves coupling; making `core/query` own claims mixes discovery with lifecycle.

### 2. Explicit registration, scope identity and operation policies

Registration accepts a caller-owned `registration_key`, distinct from the display name. Under the store metadata lock, repeating the same key returns the same app identity when settings agree; changed settings require an explicit revision-checked mutation. Protected registrations still require proof for private results and must never reissue a credential merely because the key is known. Different keys may share display names.

Every claim ensure requires a nonempty opaque `scope` string (at most 4096 UTF-8 bytes). Compare literally; callers such as Powerspec decide how project roots map to scopes. No implicit current-directory inference and no extraction from context. Queries may omit scope to search all visible scopes in one app or provide an exact scope. Context remains the existing bounded string map and does not define identity.

Use a single domain policy evaluator in `core/claim/policy.rs`: caller context, action, resource descriptor and registered policy produce allow or `access_denied`. Credential enforcement checks identity only when enabled; it neither grants blanket permission nor switches restrictions off. With credentials disabled, explicit app sessions still evaluate that app's restrictions. This is application consistency enforcement, not isolation against a hostile same-OS-user process.

Keep the first policy model small: typed action sets, exact source UUID allowances and explicit scope allowances; no wildcard ownership or general IAM expression language. Omitted restrictions permit supported app operations within that app's ownership boundary. An explicitly configured empty allowance permits none; configured allowances intersect, so an action must satisfy both source and claim/scope restrictions. A match on one dimension cannot override denial on another. Optional public exposure is a separate explicit scope set and defaults to none. Ordinary display filters remain presentation filters, distinct from authorization.

App-facing source acquisition, ensure/reuse (including cache hits), query, direct claim/output access, update, recovery, release, activation and removal use the evaluator. Carry app context through direct UUID paths; it cannot silently fall back to unrestricted store access. Store-administration APIs/CLI operations are an explicit trusted local boundary, not a mode selectable by an ordinary app request. Policy registration changes belong to that administration boundary and cannot be performed by restricted app operations to self-escalate. Administrative access is not a substitute for OS filesystem permissions.

Keep `query-claims` explicitly app-scoped. Add a separate `discover-claims` operation for generic lookup: evaluate public exposure and applicable read/discovery restrictions before matching or returning candidates. A scope absent from the public set is excluded even when credentials are disabled. Public exposure grants discovery eligibility only, not acquisition, mutation or a new claim. The generic anonymous endpoint omits candidates whose credential requirement it cannot satisfy. Context matching never creates authority. Return permitted results in app/claim UUID order with per-app observed revisions under one metadata lock.

### 3. Ensure semantics and bounded coverage

`ensure-claim` is the canonical app acquisition operation. Existing app-acquire entry points delegate to it with explicit scope. Identity includes application, scope, normalized acquisition intent (source, selector, selection), and compatible effective policy. First check under coordination, acquire outside it when necessary, then recheck the app/registry revision and reuse candidates before publication. Concurrent identical calls return the same claim UUID. Output policy `new_entry` explicitly acquires under normal source freshness and can publish a new output and advance that claim; it never means a new claim. Under reuse policy, existing available claims are returned without implicit refresh; explicit update handles moving branches/releases.

An existing claim can cover a narrower request only when the provider and selected immutable content establish coverage, the source/selector semantics and effective policies agree, and the only difference is a safe contained folder projection. Initially support natural local-directory and Git exports with matching provenance. Do not infer coverage between raw and extracted outputs, differing binary filters, moving selectors with incompatible intent, or arbitrary wildcard expressions. Output manifests validate projected paths. An ensure result includes the existing claim UUID plus a relative view path and resolved output UUID; that view is not another stored claim. Subsequent view leasing validates the projection against the claim's then-current output.

Exact identity takes precedence; otherwise choose the most specific covering folder deterministically. Prefixes are path-component based (`a/b` does not cover `a/bc`). Traversal, absolute paths, links escaping the output, and absent projections fail. If a new broader request would subsume an existing narrower claim, return `claim_overlap` without publishing a duplicate; this change does not silently replace old UUIDs or broaden old handles. Callers can explicitly release/reorganize claims while existing leases remain valid. Same-scope overlapping requests with incompatible policies also conflict; different scopes remain the mechanism for independent ownership.

Repeated ensure with identical context is a no-op. Different supplied context on a reusable claim returns `claim_context_conflict`; callers use the existing revision-checked context replacement API. Silently merging labels would erase another caller's meaning. Disjoint requests can have identical labels. Covered access uses the owning claim's context rather than inventing invisible per-view labels.

### 4. Registered policy contracts

The authenticated app registry stores a typed policy registration: defaults plus the permitted override values/bounds. Claim records store overrides and the evaluated effective policy. Validate before provider or filesystem acquisition effects. Unknown types and out-of-bounds overrides fail. Registry changes use expected application revision and reject changes that would invalidate existing claims; no retroactive silent reinterpretation.

Add `{ "type": "reclaim_policy", "strategy": "report_unavailable" | "restore_exact" | "refresh" }`. Default to `report_unavailable`, preserving explicit network side effects. App registration can select another default and restrict allowed overrides. This governs recovery when ensure/explicit recovery finds unavailable selected content. Ordinary query, bare path lookup, and lease calls remain read-only and report unavailability.

`restore_exact` uses saved immutable provenance and validates content digests, file manifest, and dependency identity before publishing a fresh output UUID and moving the claim with CAS. Git uses the saved commit, release assets use recorded identity and digest, and local/URL bytes must still match recorded content. Unobtainable old content yields `exact_content_unavailable`, never an implicit refresh. `refresh` reevaluates the saved request under normal source freshness, explicitly allowing advancement. Both preserve the old reclaimed output's identity and history. Recovery does not activate a binding; managed recovery/update still requires normal probes and coordinated activation.

The low-level `reacquire ENTRY` continues to mean explicit rerun of the saved output request; document that it can advance moving selectors. Claim recovery is the policy-governed path. Eviction continues to use effective retention demand across output records and registered claims, with default latest ten. Source refresh remains independent.

### 5. HMAC integrity and independent opt-in proof

Use HMAC-SHA256 for tamper evidence, with an OS-keyring master key scoped by store UUID. Derive separate record-signing and credential-verifier keys using HKDF-SHA256 purpose labels. The MAC covers identities, kind, generation and payload; use the library's constant-time tag verification. Unkeyed file digests remain useful for content verification and registry membership but cannot replace a trusted MAC. Legitimate mutations go through coordinated Saucepan writes and create new tags. Never automatically bless a mismatched record by recomputing its tag.

No encryption is implemented or required. Claims, contexts and policy JSON are readable; applications must not store secrets in labels expecting confidentiality. Actual signing keys and optional bearer tokens stay out of records, logs and recovery journals. Keyring is required for metadata integrity, independently of whether an app requires a credential. Missing keys fail with `key_unavailable`; opening existing data must not silently generate a replacement or fall back to an unsigned store. Fresh initialization provisions the key once with recoverable identity handling.

Application credentials remain optional and separate from the signing key. When enabled, claim reads/searches/mutations/access and private lifecycle operations require valid proof in addition to policy permission. When disabled, callers need no proof but still follow source and claim restrictions. Enabling, rotating and disabling proof are explicit coordinated policy changes; changing an enabled mode requires current proof and trusted registration authority. Rotation invalidates old proof without revoking already-issued native leases. No app UUID, query match or plain checksum serves as proof.

The old repository's byte-oriented HKDF, HMAC, randomness and zeroizing-buffer helpers can be adapted without old token/index decoders. Remove the previously introduced AEAD dependency and seal/open helpers. Native keyring packages provide OS custody; Rust 1.89 remains the dependency baseline. Production never silently falls back to a plaintext key file; tests use explicitly injected ephemeral keys.

Tokens use native secret arguments or an explicit protected CLI credential file, never literal command-line tokens, query JSON or sidecars. Registration delivers a token only to the selected secret destination, and retry cannot redisclose it. Errors redact real secrets while ordinary metadata stays readable. OS permissions control direct file access; anyone with equivalent access to the signing key can forge records, and valid complete-store replay cannot be detected without an external monotonic anchor.

Maintenance validates committed HMAC-protected intent without client tokens. Shims may launch only the verified committed binding through the existing lifetime mechanism; they receive no general claim authority. App-scoped exact-output access still checks policy. Explicit trusted store administration retains shared output maintenance without representing it as an application claim grant.

### 6. One coordinated durable transition

Retain the existing metadata lock and app revision as the coordination boundary. A commit publishes app metadata, registry membership/digests, HMAC-protected member writes/deletions, optional output publication, and lifecycle state together through a recoverable authenticated journal. Persist readable metadata with verified tags, including prepared receipts, retired registrations, temporary files and pending transactions. Never persist bearer tokens or signing keys in those artifacts. Replaying a committed transaction must be idempotent and verify envelope identities/revisions before publication. Queries observe one generation under the same lock.

Uninstall revokes access and atomically retires the registry with the live app record; receipts keep only necessary integrity-protected recovery information. Claim release journals membership removal and member retirement. Missing/corrupt registries must stop reclaim rather than be interpreted as zero retention demand. No implicit empty registry repair. Full-store rollback detection needs an external monotonic anchor and is outside this change; document this limit.

### 7. Rust resource lifetime and error boundary

Reuse the existing Rust `ResourceLease` guard for authorized claim access and contained views. The guard owns the native lock until it is dropped; a returned path alone does not retain ownership. Failed access must release partially acquired resources. Keep the dedicated CLI/shim inherited-handle contract for external execution, including protection after unexpected wrapper exit. No foreign-language binding or new lifetime broker is introduced.

Exercise registration/session, ensure, context query, explicit recovery, and exact/claim view leasing through Rust integration fixtures and CLI subprocess tests. Model two project scopes and nested resource reads while native guards are held. This verifies the integration contract without adding a separate SDK or migrating Powerspec.

Introduce native typed error codes and a CLI `--error-format json` option usable with explicit root. JSON errors go to stderr as `{ "error": { "code", "message", "details" } }`; messages can change, codes have stable meanings. Cover validation, authentication, key service, integrity, unknown app/claim/output, unavailable content, exact restoration failure, revision/context/policy/overlap conflicts, access denial, source failure, I/O, and unsupported stores. Preserve successful stdout schemas, help/version behavior, lifecycle outcome exit 2, and child stdout/stderr/status forwarding. Structured lifecycle reasons use the same codes. Rust callers inspect typed errors and CLI consumers inspect error codes; neither needs to classify English strings. Credentials and signing keys are redacted from diagnostics; responses enforce configured metadata visibility.

### 8. Portable utility plan

| Helper | Contract and effects | Reuse and verification |
| --- | --- | --- |
| Secret bytes and derivation | `random_bytes<N>()`, `derive_key<N>(secret, salt, purpose)` return zeroizing bytes; explicit errors, no persistence. Portable to another integrity-checked local cache. | Adapt old byte-only helper using vetted dependencies; RFC derivation vectors, invalid output sizes, distinct purpose/salt, failure propagation. |
| Record and capability verifier | Byte-oriented HMAC generation/constant-time verification using a derived purpose key; no token file I/O. | Reuse mature HMAC APIs; known vectors, wrong lengths/keys and altered data. Another project can validate local bearer capabilities. |
| Existing persistence/path/lease helpers | Reuse atomic file publication, `beneath`, `LockGuard`, and `ResourceLease`; preserve current ownership and timeout semantics. | Extend only for demonstrated transaction/path needs; isolated crash/replay, traversal, cross-process contention and native consumer process-exit tests. No duplicate generic lease framework. |

OS key custody, record signing-input construction, claim coverage, policy evaluation, and recovery orchestration belong in `core/claim`, not utilities. Native Rust crates supply OS credential storage directly. Domain-neutral crypto verification precedes authenticated registry integration; Rust library and CLI integration tests then exercise actual callers.

## Risks / Trade-offs

- HMAC verification requires OS key custody even when app proof is disabled -> explicit test key provider, `key_unavailable` on failure, no unsigned fallback.
- Folder coverage can accidentally broaden access or change path meaning -> explicit view paths, component containment, provenance checks, and conflict on reverse subsumption instead of silent identity rewrites.
- Moving claims affects every transaction path -> fault injection for member/app/receipt boundaries, full lifecycle acceptance, and all-app reclaim tests.
- HMAC detects modification but does not hide records or prevent key-holder forgery -> document readable metadata, OS permissions, and whole-store-rollback limits.

## Migration Plan

Implement verified dependency-ordered checkpoints from `tasks.md`; keep active OpenSpec planning artifacts outside implementation commits. New builds recognize the new contract explicitly and reject old/previous inline-claim roots without modifying them. No implicit migration and no compatibility promise for earlier binaries. Preserve `mock/` byte-for-byte and use fresh disposable roots for acceptance. Rollback means using the previous binary with its untouched previous root; do not run old binaries against the new root. Consumers must adopt the new API deliberately, and shims must be rebuilt/installed as part of a new-contract installation.
