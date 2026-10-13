# Tasks

Each numbered group is a coherent checkpoint: complete its implementation, focused tests and documentation before dependent work. Inspect the diff and stage explicit paths; checkpoint commits require authorization and the applicable commit-authoring skill. Keep active change artifacts outside implementation commits under the existing session convention. Do not push, run hosted workflows, edit Power Eagle or migrate user stores.

## 1. Typed definitions and pure evaluation

- [x] 1.1 Add leaf policy records, reference lists and derived evaluation/provenance types under `models/policy`, covering the design's type/target inventory; verify strict round trips and rejection of unknown fields/types, duplicate IDs and recursive references in focused contract tests.
- [x] 1.2 Implement pure resolution under `core/policy` with request/app/source/built-in behavior precedence, claim-pinned choices, intersected authority and constraints; verify omitted versus empty allowances, required protection, singleton conflicts, wrong targets and attempted request grants with table-driven tests.
- [x] 1.3 Preserve the existing defaults and reject conflicting cache-only/due-update, fresh/stale and unmanaged/revision combinations before effects; verify evaluated plans and zero-effect preflight fixtures, including independent payload/path verification targets.
- [x] 1.4 Document the typed catalog, target matrix, precedence and immutable UUID rule in a new policy resource guide; verify its JSON examples deserialize through the new models and run focused policy tests plus formatting before checkpointing.

## 2. Verified policy catalog and associations

- [x] 2.1 Add the policy record kind/path to existing metadata verification, catalog membership and journal publication; implement immutable batch registration and read-only inspection, verifying identical retry, concurrent UUID conflict, tamper/substitution/missing-member rejection and interrupted publication in basic and deterministic protected fixtures.
- [x] 2.2 Add internal reference-bearing app/source/claim state and source attachment revisions alongside the buildable existing front end; verify atomic association replacement, stale no-op rejection, invalidated-survivor refusal and unchanged provider cache/context/selection on failure.
- [x] 2.3 Capture definition digests and relevant attachment revisions for preflight/commit fencing; verify that an adoption during preparation conflicts while an unrelated policy registration does not invalidate the operation.
- [x] 2.4 Document policy record layout, retained-definition lifecycle and explicit adoption; verify policy definitions survive output reclaim/uninstall and the relevant metadata and adoption tests pass before checkpointing. Internal transition adapters in this checkpoint are not a supported new public API.
- [x] 2.5 Implement exact UUID lookup for verified persistent and caller-owned catalogs, returning full typed definitions; verify unknown versus corrupt/missing expected records, no provider/adoption effects, and in-memory lookup without store access. Document native lookup alongside catalog registration.

## 3. Retrieval and copied-output execution integration

- [x] 3.1 Route retrieval cache, freshness, stale and payload verification choices through evaluated plans; add portable native catalogs and preserve metadata-only registered fresh paths, verifying no cache access/writes, no implicit recovery and no home/keyring access for inline fresh operations.
- [x] 3.2 Resolve ordered release filter references and selection strategy without changing asset ambiguity/platform semantics or cloning release sources; verify reordered remote asset enumeration, wrong filter types, shared discovery and release provenance using local fixtures.
- [x] 3.3 Route copied acquisition, identical pulls, update checks, revision retention and protection through evaluated plans; verify offline Git subpath export, changed versus unchanged subpaths, independent scope checks, history-none live consumers and the maximum surviving retention demand.
- [x] 3.4 Preserve guarded source/output paths and reject ephemeral definitions before managed publication; verify caller-owned and temporary lifetime behavior, occupied destinations, no synthetic app/claim and a clear missing-registration error with unchanged metadata.
- [x] 3.5 Update the composition guide with flat catalog/reference examples and independent policy axes; verify its native fixture flow and the focused retrieval/acquisition suites before checkpointing.

## 4. Claims, authorization and explicit adoption

- [x] 4.1 Replace registry nested-default/override validation with reference resolution and derived evidence checks; verify equivalent policy UUIDs reuse exact and covered same-scope claims without silent adoption, while incompatible overlaps fail and separate scopes retain independent claims.
- [x] 4.2 Integrate explicit claim behavior adoption and app/source administrative attachment replacement; verify identity/context/output/binding preservation, credential opt-in, permitted values, stale revisions and tightening constraints that would invalidate surviving claims.
- [x] 4.3 Route query, direct access, recovery, update, lifecycle and shim paths through resolved authority while retaining their existing distinct boundaries; verify denied cache hits, historical replay, caller-catalog grant attempts, private claims and scope-independent source discovery.
- [x] 4.4 Coordinate adoption with in-flight probes, context edits, release and uninstall; verify deterministic race tests preserve prior selections and successful-check evidence, and ensure unrelated catalog changes do not cause false conflicts.
- [x] 4.5 Update claim policy, integrity and scoped-consumer guides for adopted references and derived evidence; run the focused claim/security/query/lifecycle tests using basic or injected custody before checkpointing, reporting optional native custody as not run.

## 5. Canonical public contract and CLI cutover

- [x] 5.1 Switch public registration, source, claim, retrieval, acquisition and path-policy DTOs to references/catalogs in one buildable integration slice; update all test builders and remove authoritative nested policy/override/eviction adapters, verifying strict rejection of old and mixed payloads and no second policy merger remains.
- [x] 5.2 Add native/CLI registration, inspection and revision-checked adoption entry points, plus optional sidecar catalogs for fresh operations; verify real subprocess JSON results, errors, proof handling and no successful stdout on failure. Expose relevant attachment revisions so clients can adopt explicitly.
- [x] 5.3 Retain simple CLI defaults and anonymous source queries without app UUID or query files; normalize retained convenience flags through the common resolver and make visibility convenience changes publish immutable records plus associations atomically, verifying flag/reference conflicts and derived visibility output.
- [x] 5.4 Introduce the flat-policy catalog discriminator and reject earlier nested-policy roots before recovery or key provisioning; verify old-root byte preservation, mixed-layout rejection, both integrity modes, fresh-root initialization and refusal by the prior binary where available.
- [x] 5.5 Reconcile README, CLI help and live source-query/claim/resource-access/composition guides with the canonical input shapes and fresh-root requirement; verify documented examples against the built binary and label prior verification reports as historical where necessary before checkpointing.
- [x] 5.6 Expose `saucepan policy POLICY_UUID` with default/explicit root and optional app identity using the native lookup; verify full JSON definition output, malformed/unknown IDs, no required query file/app registration for local inspection, app-bound authorization and no fallback. Add a runnable lookup example to CLI help and the policy guide.

## 6. Integrated consumer and contract verification

- [x] 6.1 Add a fresh-root Power Eagle-shaped local Git fixture covering policy registration, credential-free scoped registration, claim ensure, referenced updates/revisions/reclaim, guarded copies and release; verify the flow without editing Power Eagle or presenting a bare managed path as a lifetime bridge.
- [x] 6.2 Extend that acceptance flow with a new-policy UUID adopted by only one of two scopes and a root-free no-store caller catalog; verify unchanged other-scope behavior, no duplicate claims, immutable historical evidence and absent cache/policy/app writes in the root-free flow.
- [x] 6.3 Run `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets` and strict OpenSpec validation after integration; record exact platform/results and unavailable native/platform checks without triggering paid CI or claiming unrun environments passed.
- [x] 6.4 Record final policy-composition evidence and review live docs/delta specs against the implemented flat contract and predecessor behavior; verify no unresolved inline-authority examples remain, report checkpoint status, and document the required predecessor-first sync/archive order without performing either archive.
