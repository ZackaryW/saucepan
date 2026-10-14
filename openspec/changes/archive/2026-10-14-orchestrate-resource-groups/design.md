# Design

## Context

See proposal.md for motivation. The implemented `core/lease/group.rs` admits 1–128 existing output/claim views and retains exact outputs; it deliberately has no preparation effects. `core/claim/referenced.rs` resolves immutable policy references before `ensure/updates.rs` retrieves and publishes one claim. `core/store/updates.rs::publish_acquisition` currently combines materialization, selected-check state and publication. Calling these public methods in a loop cannot make several claim changes atomic.

`core/index/mod.rs::Journal` already accepts multiple records and registries, while `Metadata::publish_staged` commits a verified catalog generation and recovers interrupted writes. Its optional content stage handles one output. `RegistryState::encode` already encodes an entire application's claim membership. These are the transaction authorities to reuse.

`core/store/reclaim.rs::reclaim_locked` runs under store coordination and already excludes active protected bindings, pending lifecycle candidates and live leases. `tests/reclaim.rs::latest_ten_overrides_claims_and_reacquisition` confirms an ordinary claim alone does not prevent deletion. Add explicit pin membership to this eligibility check; do not overload revision counts or `useable_app`.

## Goals / Non-Goals

**Goals:** Save resource intent independently of live guards; prepare a bounded group; publish one coherent selected generation; optionally pin exact outputs beyond process lifetime; reuse current policy, claim matching and consumer lifetimes.

**Non-goals:** Destination sync pairs, runtime copy claims, lifecycle hook engines, network-wide transactions, background polling, dependent task graphs, service supervision, shell/hook rollback, an IPC lease daemon, cross-application claim updates, temporary-output group registration, legacy schema migration and external app adapters. Registration alone never pins outputs; a separate explicit pin operation does.

## Decisions

### 1. Separate group management from lease admission

Add `models/group/{mod,request}.rs` and `core/group/{mod,registry,prepare,publish,access}.rs`. Keep native guards and process launch in `core/lease` and existing consumer modules. CLI wiring belongs in a dedicated group-management module beside `cli/group.rs`.

A persisted record at `groups/<uuid>/index.json` contains UUID, owner (`local` or one application UUID), registration key, monotonically increasing revision, definition and optional selected generation. Registration keys are owner-local, literal nonempty strings bounded like explicit scopes (4096 bytes). Identical registration converges; divergent content conflicts. Display names are not identities. Lookup by UUID or key never infers an application. Replace/remove require the expected group revision; app mutations also fence the application revision. Replacing a definition clears its current generation but preserves independent claims and live leases. Removal deletes only the group, not its sources, claims or outputs. App uninstall retires its owned group records in the existing recoverable retirement transaction.

Definition members reuse group name/path limits and have a tagged target:

- `output`: an existing managed output UUID.
- `claim`: an existing claim UUID under the group's explicit app.
- `acquire`: registered source UUID, selector, reference-only selection, policy_refs, and optional claim intent (`scope`, context). Local acquisition members omit claim intent; app acquisition members require it so subsequent admission uses the existing owned-claim authorization contract. No synthetic app or scope is created.

An existing claim target selects its current output and can use its saved request for permitted recovery; automatic moving updates require an acquisition intent. Exact output targets are fixed and fail if unavailable. Registration validates structural and authority rules but performs no provider work. Persistent intents refer only to registered policies and sources, never a caller catalog that can disappear. Existing inline lease requests continue to accept their portable catalogs.

Example local definition (UUIDs denote registered resources, not embedded definitions):

```json
{
  "registration_key": "zush/project-a",
  "policy_refs": [],
  "members": [
    {
      "name": "command",
      "target": {
        "type": "acquire",
        "source_uuid": "11111111-1111-4111-8111-111111111111",
        "selector": {"type": "ref", "reference": "main"},
        "selection": {"folder": "commands/build"},
        "policy_refs": []
      }
    },
    {
      "name": "hooks",
      "target": {"type": "output", "uuid": "22222222-2222-4222-8222-222222222222"}
    }
  ]
}
```

The selector uses the existing `Selector` serializer; contract fixtures will parse examples using the real models.

### 2. Keep policy roles flat and explicit

Group `policy_refs` accept only access-policy restrictions and a path-verification floor. Preparation evaluates these restrictions for each actual action (acquire/update/recover), then admission evaluates lease; a lease-only allowance cannot authorize retrieval. These restrictions intersect current source/app authority and never grant permissions. Scope witnesses come from explicit claim intent or a real owned claim, never member names. For local output members there is no scope witness.

Behavior belongs to each acquisition member's existing flat `policy_refs`, using normal source/app defaults, constraints and adopted claim behavior. There is no nested override dictionary or group default-precedence tier. Existing immutable policy IDs change only through explicit adoption. Replacing a group cannot implicitly change a reused claim's policies or context; callers adopt those separately and then prepare again. Registration and definition replacement under an app use existing acquire authorization per acquisition intent and read/lease authority per fixed target; removal uses release authority for the affected members. Unsupported policy roles fail early.

### 3. Prepare candidates, then atomically publish selections

Under short metadata coordination, resolve owner, group revision, members, participating claim identities, current selections, effective policies and relevant policy fences. Validate every member before provider work. Share the existing claim matching rules: exact/covered requests reuse identity; broader or incompatible overlaps fail. For two fresh group intents that overlap, determine coverage before effects, select a compatible containing owner once and derive narrower views, independent of input ordering. Do not override an already existing narrower claim. Each explicit scope remains independent.

Acquire bounded intent coordination in deterministic key order outside the global metadata lock. Reuse one operation deadline for lock waiting. Refactor the current single-resource path to expose an internal candidate result containing validated output, owned guard, proposed acquisition-check update, and optional claim mutation. Candidate preparation must not publish claims, acquisition success timestamps or selected group state. It may publish complete unclaimed immutable outputs and source cache records individually using the current single-content transaction. That deliberately keeps content publication recoverable without rewriting the journal for multiple filesystem renames.

Retain every candidate against reclaim until commit and handoff. For app callers, candidate preparation preserves the captured authorization context without making an unauthorized direct-output API call or forging temporary claims. Do not call public `ensure` repeatedly or temporarily persist claims to satisfy a lease check.

For compatible requests to the same Git source/selector, resolve once to an exact commit and materialize subpaths from that source handle. Due updates of that shared resolution are combined before retrieval; incompatible retrieval-policy choices remain separate and report their exact provenance. Release each source guard before starting another retrieval that would contend on the same provider lock. The per-member update policy still decides which existing outputs move. A manual member can intentionally remain on its previous revision.

At commit, reacquire metadata coordination and recheck group revision, current application/claim observations, source-policy attachments, definitions and candidate availability. Merge only affected claims into the latest registry, increment the app revision once, and publish the new group generation, acquisition success records and all affected claim records in one `Journal` transaction with no content stage. A no-op reuses the existing generation unless successful check evidence changes. Selected generation entries include exact output UUIDs, claim witnesses/relative views and observed provenance; never just a list of live claim IDs.

Failure leaves the prior generation, claim selections and successful-check state untouched. Earlier complete cache/candidate outputs can remain reclaimable; failure does not promise to undo downloads. A journal error after durable commit is an indeterminate response, resolved by normal recovery and reading the group revision; never blindly retry `new_entry` work. Concurrent release, adoption, uninstall or another prepare causes a bounded conflict instead of resurrecting stale state.

### 4. Preserve update, recovery and revision meanings

Preparation is explicit and evaluates each existing update policy (manual by default) and source refresh policy (24 hours by default). A newly registered or missing acquisition member is acquired automatically. Missing claimed outputs follow the existing reclaim policy: report unavailable, restore exact, or refresh. Any member failure fails the complete new selection; permitted stale fallback is visible per member and never reports a failed origin refresh as successful. Revision policies continue governing output history, not group registration. Current group references do not act as permanent GC pins.

Persistent groups in this change require managed outputs and therefore cached retrieval. Temporary/no_store policies are rejected before provider work, consistent with current persistent claim and managed-output composition. Existing temporary/caller-owned flows stay supported independently. This is an explicit bound on persistent orchestration, not a global requirement to cache or claim everything.

### 5. Consume a selected generation without silently updating it

Native APIs expose register/read/list/replace/remove/prepare/open operations. CLI commands are `register-group FILE`, `show-group UUID`, `list-groups [--key KEY]`, `replace-group UUID FILE --expected-revision N`, `remove-group UUID --expected-revision N`, `prepare-group UUID`, `run-registered-group UUID -- PROGRAM ...`, and `prepare-run-group UUID -- PROGRAM ...`. Existing `run-group FILE` is unchanged. Standard root/app options remain shared; credentials are never stored in definitions.

Preparation returns a lifetime-bearing native prepared group; ordinary CLI preparation returns metadata only and documents that UUIDs/paths are not pins. Explicit prepare-and-pin instead retains paths through its durably published pin as described below. Prepare-and-run must transfer the exact prepared generation's guards into launch without a publish-to-lease reclaim gap. Open-only uses coordinated current authorization and the saved generation; if a witnessed claim moved or vanished, report revision conflict/unavailability and require preparation. Never silently mix current individual claims with an older group generation. A local fixed output remains accessible without app context; an app-owned group never falls back to local access.

Group launch reuses the existing descriptor and inherited lifetime handles. Group removal, future updates and app uninstall cannot redirect a group already in use. Native/CLI metadata lookup exposes availability and selected provenance but is not a capability token or a file-access sandbox.

### 6. Durable pins retain exact outputs without copying

Add `models/pin` and `core/pin` for bounded pin requests, verified records, admission, lookup and release. This feature remains independent of app claims and group registration. A local caller can pin an existing ready output with no app identity or credentials; an app-bound pin must first satisfy current output/claim access and the new `pin` action. Add `unpin` as a separate action so restricting new retention does not necessarily prevent cleanup. Neither action grants filesystem access or broadens source/scope authority.

A pin record at `pins/<pin-uuid>.json` has one owner (trusted local or app UUID), an owner-local registration key, a fixed set of 1–128 distinct output UUIDs, explicit policy references, source/scope witnesses where applicable, optional group/generation provenance and state `active` or `released`. Store membership, checksums/HMAC and the existing journal verify this record. A UUID membership set is the authority; no mutable persisted reference counter is needed. Same owner/key and equivalent target/policy request is idempotent; different content conflicts. Different consumer keys create independent pins even for the same output. Output identities and policy associations on an existing pin never change. A released record remains a receipt; reusing its key cannot silently repin content. Another pin needs a new key.

Use an immutable leaf policy to make the opted-in retention choice explicit:

```json
{
  "uuid": "33333333-3333-4333-8333-333333333333",
  "type": "pin_policy",
  "retention": "until_released"
}
```

The direct pin request is:

```json
{
  "registration_key": "power-eagle/session-42/plugin-example/revision-a",
  "outputs": ["22222222-2222-4222-8222-222222222222"],
  "policy_refs": ["33333333-3333-4333-8333-333333333333"]
}
```

Exactly one registered pin_policy is required on a pin request; it is not inherited from source/app defaults, an ordinary claim, or group definition. Other policy roles are rejected on this request. Current authority and existing path-verification requirements still apply at admission. No nested policy object, implicit pin, TTL, heartbeat or keep-all revision count is introduced. Pin policy is not an acquisition protection enum: it creates separately owned membership, rather than rewriting shared output evidence. Ordinary acquisition and inline group leases reject a pin_policy in their existing roles.

Validate all targets as ready under coordination, acquire temporary guards in deterministic order, perform required scans outside the global lock, and recheck availability and relevant authority before publishing the complete pin set. Return paths only after durable pin publication. Reclaim and pin publication serialize through the same store lock: reclaim winning first yields unavailable content; pin winning first prevents deletion. Never pin a reclaim_pending/reclaimed output, recover it silently or publish a partially protected set.

Native/CLI operations include `pin-outputs FILE`, `pin-group GROUP_UUID --expected-revision N --pin FILE`, `pin PIN_UUID`, `pins [--key KEY]` and `unpin PIN_UUID`. The group pin file contains only registration_key and policy_refs; outputs are derived from the exact saved generation. A group pin snapshots its outputs and retains those identities independently of future claim movement. A missing/stale generation fails rather than pinning a mixture. `prepare-group ... --pin FILE` and the corresponding native operation create the pin in the same final metadata transaction as the prepared generation and claims; before that commit, candidate guards provide continuity. A failure publishes no pin or selected state. Existing prepare/run behavior remains unpinned unless that option is explicitly supplied.

For prepare-and-pin, store the canonical operation request and expected group revision with the receipt. A retry using the same key/request returns that committed exact result after current authorization, without repeating provider work or new_entry acquisition; it does not claim to represent a later current generation. Another request under that key conflicts before effects. A genuinely new update uses a new key. Post-commit errors are resolved by receipt lookup, as with generation publication.

Pin lookup returns owner, state, exact output paths/availability, policy evidence and origin generation. For app owners, lookup rechecks current read/source/scope permissions against the saved witnesses; it does not require the mutable claim to still select that output. A pin is retention evidence, not historical permission to bypass current restrictions. Release verifies the owner and current `unpin` authority but does not require content availability or a still-live claim, allowing cleanup of obsolete pins. Trusted local administration can inspect/release an orphaned pin explicitly. Repeated authorized release of a known pin is a no-op; unknown IDs fail. Claim release, group replacement/removal, app uninstall and policy changes never implicitly release a pin. App uninstall leaves its pins inspectable to local administration; it does not create an expiry policy disguised as cleanup.

Every active pin excludes its exact outputs from revision reclaim, including history none. One owner's release cannot remove another owner's protection. Unpin changes retention metadata only; it neither deletes payload immediately nor overrides leases, active bindings or other policy demands. Future pins remain subject to current authority. External edits/missing files still fail normal verification; a pin protects against Saucepan reclaim, not arbitrary filesystem changes.

Power Eagle's resulting flow is **prepare/acquire → pin A → read store/A directly → prepare and pin B → activate B → unpin A after all A users finish**. Output paths never redirect. Content changes create B; unchanged-content reuse can keep A under existing identical-pull semantics. Mutable runtime data stays outside acquired payloads. This removes the need for a duplicate destination, synchronization baseline, parent-process lease bridge or phase hook merely to retain plugin bytes. The host still owns plugin activation and the decision that an old version is no longer used.

### 7. Utility and dependency plan

No new generic helper or crate is justified. Application grouping, pin ownership, claim matching, candidate planning and transaction assembly remain domain code.

| Mechanic | Reuse and actual contract | Verification |
| --- | --- | --- |
| Typed bounded inputs | Existing serde tagged enums, UUID parsing, PolicyRefs and CLI byte limits | Reject unknown fields, duplicate names, invalid views, conflicting aliases and 129 members |
| Deterministic guard acquisition | Existing group admission sorts UUIDs, applies one remaining deadline and releases RAII guards on failure; use the same mechanism with intent keys where required | Reversed order, timeout after a later lock, no leaked guard |
| Content preparation | Existing retrieval handles and `composition::prepare` plus a narrow split of `publish_acquisition` | File manifests, copy isolation, shared Git commit, unchanged single-resource semantics |
| Metadata publication | Existing `Journal` multi-record fields, `RegistryState::encode`, `Metadata::publish_staged(..., None, ...)` | Fault injection before/after journal/catalog publication; never mixed selections |
| Durable retention membership | Verified catalog records and `BTreeSet<Uuid>` collected under the existing reclaim lock | Independent pins, restart, unpin versus reclaim, corrupt pin member and history-none exceptions |
| Process lifetime | Existing `GroupLease`, descriptor workspace and `core/lease/launch` | Literal args, child failure, wrapper death and old-generation retention |

These APIs are already present and tested; direct reuse avoids new independent authorities. A generic transaction crate or embedded database would not supply Saucepan catalog authentication, record identity or recovery semantics. A graph crate is unnecessary for a flat bounded member list. An asynchronous runtime and parallel-fetch framework add scheduling semantics without a present requirement. Existing serde/uuid/std/tempfile/native locks remain sufficient; no dependency installation or speculative utility scaffolding is planned. If extracting a truly portable deadline/ordered-lock helper becomes necessary, its contract must use ordinary keys and callbacks with domain-neutral failure tests before integration, rather than wrap group policy in `utils`.

## Risks / Trade-offs

- Prepared bytes can outlive a failed operation → report this explicitly, retain no false claim/check success, and leave complete candidates to normal reclaim.
- Persistent group metadata can outlive reclaimed payload → lookup reports availability; only explicit preparation can recover; leases still pin actual use.
- Independent claim updates can stale a group → snapshot witnesses reject inconsistent opening; explicit preparation reconciles them.
- Holding provider locks across several member preparations can self-contend → resolve/share compatible material and release handles before incompatible retrieval attempts.
- Consumer runtimes can close inherited handles or detach services → document supported lifetime contracts; group orchestration is not a daemon or supervisor.
- An in-process host cannot hold a short CLI lease → explicitly pin its immutable outputs and release after use; do not duplicate payloads just to extend retention.
- A crashed consumer can leave durable pins → expose owner/key/output inspection and explicit release; never guess that a pin is abandoned from elapsed time or process absence.
- Cache/metadata publication is atomic only at its documented boundary → tests distinguish final selection atomicity from downloads, external hooks and consumer installation effects.

## Migration Plan

Add the new verified group/pin identities, pin policy/action validation and path handling to the current flat contract. No legacy decoders, automatic conversions or compatibility shims. Existing current-contract roots without groups/pins require no rewrite. Older binaries are not promised support for new records. Verify basic and injected-key protected roots, crash recovery and single-resource regressions using fresh isolated roots; preserve historical `mock/`. Ship docs and examples only after the acceptance checks pass. Active planning artifacts stay uncommitted until archive. Update the earlier consumer integration assessment during implementation: durable pins replace its proposed copy/IPC retention workaround for Power Eagle; real host activation still needs consumer testing.
