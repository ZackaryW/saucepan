# Design

## Context

See proposal.md for motivation and scope. `core/store/access.rs` already owns `ResourceLease`, takes a shared per-output coordination lock under store coordination, and only creates native process markers when inheritance is requested. `core/policy/access.rs` resolves path policies and fences relevant associations around verification. `core/claim/session.rs` supplies app identity; `core/claim/policy.rs` separates trusted local administration from app authorization. Ordinary request access policies are deliberately rejected in `core/policy/resolve.rs`.

`core/launch.rs::launch_leased` inherits one output handle, uses exec on Unix and spawn/wait on Windows. `core/consumer.rs` handles temporary consumer ownership and crash sweeping using native marker evidence. `tests/run.rs` and `tests/scoped_consumer.rs` exercise reclaim, wrapper death and native guard isolation. These are the extension points; generic bounded-capture `utils/process.rs` is not the interactive execution boundary.

## Goals / Non-Goals

Provide one non-serializable ownership object for a complete named selection, with consistent admission and explicit narrowing policies. Keep authority, output selection and OS lifetime mechanics separate. Admission consistency is a snapshot of existing selections, not a transaction updating their versions. Refer to proposal.md for deferred features. Arbitrary filesystem paths and temporary retrieval/acquisition handles are not additional member variants in this slice; their existing APIs remain available.

## Decisions

### Strict request and one caller context

Add `models/lease/{mod,group}.rs` for `GroupRequest`, member targets and serializable descriptors; put orchestration and guards in `core/lease/{mod,group}.rs`. Public entry points are `Store::lease_group` and `AppSession::lease_group`, taking the request and a borrowed `PolicyCatalog` (use its default empty value when no portable definitions are needed). Each operates on one store. The session supplies one app identity for every member; there are no per-member credentials or administrative overrides.

```json
{
  "members": [
    {"name":"command","target":{"type":"output","uuid":"11111111-1111-4111-8111-111111111111"}},
    {"name":"hooks","target":{"type":"claim","uuid":"22222222-2222-4222-8222-222222222222"},"relative":"hooks"}
  ],
  "policy_refs": ["33333333-3333-4333-8333-333333333333"]
}
```

The example requires an app session because it includes a claim. Without identity, only output targets are valid, using existing trusted local output access. No app registration or credentials are required for that path. Names are unique, case-sensitive ASCII `[A-Za-z][A-Za-z0-9_-]{0,63}`; 1..128 members, a bounded 1 MiB CLI request, existing 256-reference limit. Unknown fields, malformed UUIDs and unsafe relative paths fail before lease effects. Relative views are directories checked against manifest and containment; omission denotes the output root. Repeated output targets under different names are valid. A name is a session label, never an ownership scope or persistent alias.

### Flat policies form a restrictive group boundary

Reuse existing `access_policy` leaf definitions and path-target `verification_policy` definitions. The group-only policy resolver accepts these in `policy_refs` and partitions them into narrowing access predicates and a verification floor. Other direct policy types are rejected. Multiple access definitions intersect; multiple path-verification definitions conflict even when equal. No nested definitions or new group policy type is necessary.

First authorize each member with current registered authority using the existing app/output/claim rules. Then apply every group boundary to that same member with action `lease`, actual source and, for claim targets, actual claim scope. A direct output target has no group scope witness: if a boundary restricts scopes, require a claim target instead of inventing or borrowing a scope. Omitted dimensions add no restriction; empty allowances deny. This rule also applies to trusted local groups, whose caller-supplied boundary must not be short-circuited by the administrative authorization path.

Resolve registered path behavior per member using the current evaluator; group full verification strengthens metadata, while group metadata never weakens a member's full requirement. The strongest required mode across aliases of one output controls its one scan. Report actual verification and policy provenance per named view. Caller catalogs reuse current shadowing/validation rules; supplied access policies can only narrow this operation, never alter persistent grants, associations or lookup visibility. Do not relax ordinary acquisition request validation. A group guard carries paths and lifetimes, not a credential for additional operations or a sandbox.

### Coordinated selection followed by guarded verification

Under store coordination, load the verified catalog and relevant app/source state, authorize all members, capture claim-to-output selections, validate views, derive verification requirements and deduplicate output IDs. Acquire shared use guards in UUID order before releasing metadata coordination. Use one remaining-time lock budget for the group, not 128 independent 30-second waits. Refactor locked internal helpers rather than repeatedly calling public lease methods that independently snapshot and lock.

Perform payload scans outside the global lock while all outputs are protected. Re-enter coordination to check captured relevant app/source revisions and definitions before returning. A changed selection, release, uninstall, credential rotation or policy adoption during preparation produces `revision_conflict` (or the existing applicable denial), releases all guards and exposes no partial result. Unrelated catalog additions do not conflict. Do not retry indefinitely or follow a new selection implicitly. Concurrent groups use deterministic ordering; reclaim continues using its existing nonblocking use-lock checks.

`GroupLease` owns a UUID-keyed set of unprojected `ResourceLease` guards plus a name-keyed map of immutable view descriptors. Do not mutate one shared lease's directory for each alias. Dropping the group releases its distinct guards; another independent lease still protects its output. Once admitted, later policy or claim changes affect future opens, not the existing guard. No persistent group record, claim, acquisition, history entry or fresh check is created. Ordinary store-open recovery and temporary lease bookkeeping retain their existing effects.

### Child workflow and descriptor lifetime

Add `saucepan [--as-app APP_UUID] run-group FILE [--policies CATALOG] -- PROGRAM ARG...` with the existing default/explicit root and credential options. It reads the same request, obtains the group, prepares all inherited markers, and launches only after every step succeeds. It preserves literal arguments, working directory, inherited streams and exit status. Use Unix exec and Windows spawn/wait through a shared internal launch helper so existing single-resource semantics remain intact.

Set `SAUCEPAN_RESOURCE_GROUP_FILE` to a create-new UTF-8 JSON descriptor in an owned temporary workspace. Its `members` object maps names to output UUID, optional claim UUID, absolute selected directory, relative view and actual verification. It contains no credentials, signing keys or inherited handle values. The descriptor is informational and is never accepted back as lease authority. Remove inherited `SAUCEPAN_RESOURCE_DIR` and source-path variables for this child to avoid suggesting a singular or stale selection; leave the parent environment unchanged.

Use the existing temporary-consumer marker ownership for the descriptor so it survives wrapper failure alongside every inherited output marker. Handle setup or spawn failure drops all handles; closed residual markers and scratch are eligible for existing cleanup, without PID/age heuristics. Descendants deliberately closing inherited handles remain outside the documented guarantee. Do not serialize live guards or add a bare group-path command that implies retained lifetime.

### Utility plan and crate decisions

- **Native marker mechanism:** directly reuse `utils/lifetime::{inheritable_marker(Path), marker_held(Path)}`. It accepts ordinary paths/files and can serve another package manager or scratch-data worker. Preserve create-new ownership, OS errors and caller responsibility for directory coordination. Focused real-process tests cover two markers, failed later creation, inherited survival and closure on each executed host. No new multi-handle utility is needed merely to store `Vec<File>`; Rust ownership handles partial acquisition cleanup.
- **Deadline-based lock acquisition:** retain `utils/lock::LockGuard::acquire(path, mode, remaining)` and compute one `Instant` deadline in the domain coordinator. Lock paths, UUID ordering and store coordination belong to core, not utils. Domain-neutral contention tests verify the existing timeout contract where any refactor touches it.
- **Temporary descriptor ownership:** reuse/refactor `core/consumer.rs` internally for transferring a `TempDir` and its marker to a dedicated child. Saucepan scratch naming and sweeper rules are domain behavior, so this stays in core. Tests must observe descriptor survival after wrapper termination and later cleanup, not mock call counts.
- **Dependencies:** existing `serde`/`serde_json`, `uuid`, `tempfile`, standard collections/RAII and current native `windows`/`libc` primitives cover the plan. Retain `process-wrap` for its existing bounded process-tree jobs; it does not supply resource authorization or replace the interactive lease handoff. No graph/IAM engine, async runtime, Python binding or lock crate is justified by this finite group contract. No dependency change or new generic framework is planned.

The group model validates names and member bounds; `core/policy/group.rs` owns narrowing evaluation; `core/lease/group.rs` owns snapshots, deduplication and rollback; `core/lease/launch.rs` composes `core/launch.rs`/`core/consumer.rs` for dedicated child transfer. Keep these responsibilities out of CLI parsing and avoid enlarging the existing policy execution facade with group orchestration. Request and portable catalog files are each bounded to 1 MiB; the descriptor is bounded to 16 MiB.

## Risks / Trade-offs

- Holding metadata coordination while acquiring use locks can delay writers → bound the member count, use stable ordering and one deadline; verify reclaim lock ordering before integration and exercise contention tests.
- Verification can race policy/claim mutation → retain outputs and revalidate relevant revisions after scanning; do not hold the global lock for payload I/O.
- Duplicated outputs can hide a forbidden claim or a stronger verification requirement → authorize each named member before deduplicating protection and scan at the strongest required mode.
- A caller can omit its optional boundary on another unrestricted call → describe the boundary as voluntary narrowing; app/source restrictions remain the durable enforcement points. No filesystem sandbox or identity isolation is promised.
- Consumer runtimes may close inherited handles → document the supported process boundary and test a Zush-shaped subprocess fixture, including hooks/services from other outputs; do not claim unrelated grandchildren are automatically protected.
- OS-specific inheritance behavior → add Windows and Unix tests but report only actually executed hosts; no paid hosted runs are started by this change.

## Migration Plan

This is an additive API/CLI feature on the current flat store contract. There is no schema migration, version fallback, alias adapter or historical store rewrite. Existing commands stay usable. Disabling use of group APIs rolls consumers back to existing operations without removing persisted state because no group is registered. Preserve historical mock roots and external consumer repositories. A later independent change may add persistent groups, retrieval orchestration and coordinated updates after this slice is complete.
