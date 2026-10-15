# Design

## Context

See proposal.md for motivation. Repository inspection found:

- `models/policy/{record,references,requests,effective,composition}.rs` already provides typed leaf records, axes, reference-only operation requests and provenance. Defaults are cached retrieval, 86,400-second freshness, manual acquisition updates, reuse of identical content, latest-ten history and report-unavailable recovery.
- `core/policy/{catalog,resolve,prepare,adopt,persistence}.rs` already validates targets, rejects competing singleton policies, intersects restrictions, freezes adopted claim behavior and performs revision-checked adoption. Profile resolution must precede this evaluator, not replace it.
- Payload verification currently defaults to `if_available`; path/store verification has its own contract. The discussion's independent opt-ins do not mean silently changing these existing checks to `none`. Profiles leave verification axes unspecified and report their actual effective values.
- `core/bootstrap.rs` already requires a GitHub release, one executable and usable-app protection. `core/lifecycle` already provides installation, update, rollback, uninstall and durable recovery operations. A profile does not supply an executable, probe, destination, scope or source.
- `sdk/contracts/operations.json` and both clients currently exclude managed lifecycle calls. JSON transports currently treat nonzero command exits as errors; lifecycle receipts legitimately use exit 2 for non-complete results and need explicit typed decoding.
- Existing policy, composition, lifecycle and SDK fixture tests provide suitable regression seams. No active change existed when this change was created.

## Goals / Non-Goals

**Goals:** Add a small Rust-owned profile recipe layer, make resolved settings understandable, and expose explicit setup through existing lifecycle APIs. Keep policy leaves and UUID associations the sole runtime authority. Preserve low-level composability, root-free fresh operations and truthful resource lifetimes.

**Non-Goals:** Profile inheritance, user-editable named profile registries, a second persistent profile graph, profile fields in claim/output schemas, a daemon, automatic PATH changes, SDK-managed binary downloads, broad SDK transport refactoring, reader-pin delegation, new retry algorithms, consumer repository edits or legacy schema support.

## Decisions

### 1. Profiles are pure expansions with fixed leaf identities

Add `models/policy/profile.rs` for profile names, expansion inputs/results and `core/policy/profile.rs` for the built-in recipe table and pure expansion. A profile result contains `name`, `digest`, a bounded `catalog` of ordinary `PolicyRecord`s, and `retrieval_policy_refs`, `acquisition_policy_refs` and `combined_policy_refs`. The combined set is the de-duplicated union suitable for existing composed output requests and default associations; stage-specific sets avoid wrong-target failures on separate retrieve/acquire calls. The result is planning data, not an accepted new policy definition type.

Allocate checked-in UUID constants for built-in leaf definitions. The same leaf body reuses its UUID across recipes; changing any leaf body allocates a new UUID. Test uniqueness and golden definitions. Compute the recipe digest through existing canonical digest utilities over name and ordered canonical recipe contents. Names are conveniences in the installed manager; they are never dereferenced on a subsequent read or update. Existing resources persist only adopted leaf identities and evidence. Callers needing reproducibility retain the expanded records/refs rather than a mutable name. A later binary can offer a changed recipe without rewriting old registered definitions.

Alternatives rejected: persistent profile resources would add another authority and adoption graph; random UUIDs on every preview would make identical retries noisy; adding a hashing/UUID crate is unnecessary for three checked-in recipes and existing canonical digest support.

### 2. Three recipes with separate operation axes

| Setting | managed | explicit-update | temporary |
| --- | --- | --- | --- |
| Retrieval cache policy | cached | cached | temporary |
| Retrieval freshness | max_age 86,400 seconds | always | always |
| Stale fallback | false | false | false |
| Acquisition update | manual | manual | omitted |
| Identical pull | reuse | reuse | omitted |
| Managed revision history | latest 10 | latest 10 | omitted |
| Unavailable-content recovery | report_unavailable | report_unavailable | omitted |

`explicit-update` deliberately differs on retrieval: when the caller explicitly requests source retrieval/update, it checks fresh origin material instead of accepting a still-fresh 24-hour cache. It does not refresh on lookup, schedule checks or force a new revision for identical content. An explicit update operation retains its existing force-refresh semantics. Automatic acquisition updates remain an independently selected update-policy UUID; `managed` does not silently change today's manual acquisition behavior.

Latest ten means the ten newest committed revisions including the current one, subject to the existing maximum surviving demand rule. Protected entries outside that window are additional exceptions, not ten extra unpinned slots. Pins remain separate explicit requests and are never generated by a profile.

`temporary` emits no managed revision/protection/update choices and does not choose acquisition destination. The caller must explicitly choose temporary or caller-owned output using existing request fields. Temporary native handles and guarded commands retain scratch; SDKs must not expose a temporary path after its owning command exits. For a persistent caller-owned path use caller-owned delivery; for a no-store original explicitly replace the cache axis with a no-store policy and provide its destination. Incompatible combinations fail preflight.

No profile selects credentials, store integrity mode, verification, visibility, access grants, scope or usable-app protection. Unknown names fail; no fuzzy fallback. `managed` is the only default name in the profile-expansion API. Omission of profile expansion in all existing operations remains omission, without policy registration or synthetic identities.

### 3. Explicit override and registration steps preserve normal policy authority

Proposed public commands:

```text
saucepan profiles
saucepan profile [NAME] [--catalog FILE] [--retrieval-policy UUID ...] [--acquisition-policy UUID ...]
saucepan register-policies FILE
saucepan explain-policies FILE
```

`profiles` and `profile` run before home/root discovery and have no store access. The optional expansion catalog contains caller-provided flat leaf definitions for overrides; it is not a reference to persisted metadata. To use a registered custom policy, callers may explicitly look it up and provide that definition to the pure expansion call. Unknown references fail. Managed use explicitly registers returned records with the existing command, then uses returned refs. Portable use passes the returned catalog directly to existing root-free operations. Registration alone attaches nothing and retains current local-administration authorization. An app-bound caller cannot turn preview into implicit administrative registration.

For each stage, a single explicit singleton override replaces that recipe's leaf on the same axis before ordinary evaluation. Validate override targets/duplicates first; two explicit definitions on the same axis conflict even with equal values. Do not modify definitions or relax constraints. Non-profile policies, visibility and access restrictions keep their existing roles; ordered asset filters stay in their separate input. A conflict between different singleton choices across stage sets that cannot be represented by the combined request must be reported rather than resolved by list order. The final request remains an ordinary reference-only request.

Selecting a profile for a new operation contributes at that explicit request level; attaching its refs as app/source defaults uses those existing levels. It introduces no new precedence. Existing claims require explicit revision-checked adoption before using different expanded policies, and current restrictions remain in force. There is no `--profile` shortcut on every acquisition command: explicit expansion and normal references avoid hidden writes and identity ambiguity.

### 4. Explain through the same evaluator without acquiring anything

Add `models/policy/explanation.rs` and `core/policy/explain.rs`; extend `cli/policies.rs`. `explain-policies` accepts a strictly tagged input selecting `retrieve`, `acquire`, `compose` or an existing `claim`, its normal typed request/resource identifiers, and a portable catalog where already supported. App context comes from the existing session flags/client binding, never a caller-authored effective plan. Claim explanations distinguish frozen adopted behavior from current restrictions; they do not substitute current defaults for historical choices.

Return applicable axes with effective values and origins (request, application, source, adopted claim or built-in), UUID/digest evidence where present, and the restriction/constraint evidence that participates in the decision. Report irrelevant axes as not applicable rather than pretending their struct defaults execute. Recipe provenance is descriptive preview data; runtime authority is the expanded UUIDs, not a claimed profile name. Results are snapshots, not authorization tickets or promises that provider bytes/destinations exist. Actual operations repeat validation and publication fencing.

Reuse `prepare`/`resolve`/claim authority functions, extending their internal trace rather than building an independent evaluator in the CLI/SDK. Persisted inputs use one verified snapshot and applicable read authorization. Inline portable explanations remain root-free. Denied/corrupt/invalid requests return normal structured errors with no protected explanation payload. No provider fetches, claim creation, policy registration, probing or adoption; existing store-open committed-journal recovery is unchanged.

### 5. Application setup composes profiles with the current lifecycle

There are two separate paths:

1. A resource consumer previews/registers desired policy leaves, optionally registers an application with explicit scope restrictions, then uses ordinary ensure/group/acquisition calls. No executable installation is implied. Public unbound usage still needs no app UUID.
2. A managed executable caller supplies source/selector/platform selection, executable path, bounded probe, scope, shim name and built shim to the existing install operation. It uses expanded managed/explicit-update refs plus an explicitly registered `useable_app_policy`. Setup must preflight managed storage and required protection before provider work; it must not silently ignore a temporary policy. Existing no-profile calls retain their current contract.

Expose a small Rust-owned setup-policy preparation operation, `profile NAME --for-install`, which returns managed refs with the ordinary usable-app leaf included and clearly reports this extra setup requirement. With no name, it uses managed like all expansion; documentation recommends `explicit-update` for installed tools/self-management. Temporary with `--for-install` is a `policy_conflict`. This modifier does not run installation, register anything, choose credentials or invent scope. Generic custom setup remains possible through hand-assembled ordinary leaves.

For Saucepan itself the initial prerequisite is the downloaded/extracted release manager plus its matching shim. Instructions then explicitly initialize a basic root (or opt into supported keyring mode), register the chosen profile and GitHub release source/filter policies, and call bootstrap with a request referring to them. Bootstrap supplies its existing fixed `saucepan/self` scope and version probe. Routine self-update continues through the existing managed update, not an ordinary update-claim call. An inactive newly downloaded release never replaces the active binding by itself. PATH instructions are returned/documented, not executed as shell/profile edits.

No encompassing installation transaction is invented: profile/source registration can remain after failed install, and reusable cache material can survive. Only lifecycle publication has the existing recoverable all-or-nothing live app/claim/binding semantics. Reusing an operation UUID reuses the original expanded identities and payload; selecting a changed profile before retry must not overwrite that receipt and instead requires a new operation.

### 6. SDK coverage is explicit and bounded

Add equivalent Python/TypeScript operations for profile listing/expansion and policy explanation. Rust owns the definitions, composition and effective resolution; SDKs only serialize and decode.

Expose existing `install`, `bootstrap`, `update-app`, `self-update`, `rollback`, `uninstall`, `operations` and `reconcile`. Map methods idiomatically (`install_application`/`installApplication`, etc.) while retaining all explicit identities, inputs and optional operation UUIDs. Standalone shim install/replace/adopt remains outside SDK coverage. Callers still provide the actual manager executable and built shim; installing/importing an SDK does not provision either. Credential creation during managed installation is not added; existing configuration operations remain explicit.

Add a lifecycle result mode to the operation inventory and transports. Decode a valid lifecycle receipt/list from stdout on the documented exit codes 0 and 2, verify that status and exit code agree, and preserve the raw exit code alongside the typed receipt. Non-complete receipts are never presented as completed installation; JSON domain errors on stderr remain errors. Malformed receipts are protocol failures and potentially indeterminate for mutations. Non-lifecycle calls keep their current nonzero handling. No automatic retry, reconciliation, unpin, compensation or polling.

### 7. Package structure, utilities and dependencies

Keep profile/explanation data and logic in the existing policy subfolders, command handling in `cli/policies.rs`, lifecycle orchestration in current lifecycle/bootstrap modules, and SDK adapters in their existing model/client/contract structure. Avoid a new generic configuration framework or an enlarged monolithic CLI implementation.

The new logic is Saucepan-domain recipe/evidence composition, not a new cross-project utility. Reuse canonical digest, typed catalogs, bounded reference validation, payload scratch, process execution and operation journaling. Existing `serde`, `uuid`, `clap` and standard collections suffice. Do not add a profile/configuration/templating crate, SDK-side policy engine or dependency solely for UUID generation. Validate pure recipe behavior, then integrate through existing policy and lifecycle seams with focused regression cases. No generic helper requires a separate utility implementation phase.

## Risks / Trade-offs

- [Profiles appear to weaken restrictions] → Exact authorization and constraint regressions; effective evidence names current restrictions and independently chosen behaviors.
- [Recipe names drift across manager releases] → Fixed leaf UUID/body fixtures, recipe digest, explicit registration/adoption and same-receipt retry with retained expanded inputs.
- [Temporary paths outlive native ownership] → Guarded CLI/SDK tests and no standalone temporary-path return contract.
- [History wording promises automatic reader safety] → Explain the actual latest-ten window and independent pins/leases; no default pin creation or release.
- [Two managed recipes seem similar] → Document manual acquisition for both and the specific 24-hour versus always-check retrieval difference; do not imply automatic updates.
- [SDK interprets exit 2 as success or malformed JSON] → Explicit lifecycle result contract and fixtures for complete, pending, failed, conflict, domain error and truncated output.
- [First setup sounds like SDK bootstrap from nothing] → Require an explicitly obtained manager/shim, provide exact commands, and test outside the checkout.
- [Host/platform confidence is overstated] → Basic isolated acceptance first; separately report native platforms/keyring checks actually exercised. Do not dispatch hosted jobs for planning or routine checks.

## Migration Plan

Implement in verified checkpoints: pure expansion; effective explanations and CLI; existing lifecycle integration; SDK adapters; user instructions and actual-build acceptance. This is an additive current-contract feature: no old schema decoding, compatibility facades or historical profile resolver are added. No-profile calls keep their behavior by design, not through a compatibility layer. Existing apps adopt new refs explicitly under existing revision checks. Rollback of selected executable outputs uses current lifecycle rollback; removing adoption uses explicit permitted policy adoption, never rewriting immutable definitions. Leave active OpenSpec artifacts uncommitted during implementation checkpoints and do not push.
