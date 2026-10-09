# Design

## Context

See proposal.md. Inspection found these concrete integration gaps:

| Current implementation | Observed behavior | Consequence |
| --- | --- | --- |
| `core/bootstrap.rs::bootstrap` | Registers app, acquires claim, activates, installs shim separately | Failure can leave a registration/claim/binding without a complete launcher |
| `core/bootstrap.rs::self_update` | Calls `update_claim` before `activate` | A failed probe preserves the binding but moves the managed claim |
| `core/activation.rs::install_shim` | Checks executable destination, writes sidecar, copies executable | Sidecar collisions and interrupted pair publication lack an ownership/recovery contract |
| `core/index/mod.rs::Journal` | Writes records and optionally publishes an output directory | Cannot express application deletion or deferred shim maintenance |
| `core/activation.rs::deactivate` | Journaled binding-only mutation | Correct primitive; must remain distinct from removal |
| `core/store/reclaim.rs` | Uses output policies, attached claim demand, active bindings and live leases | Uninstall must remove app demand without bypassing remaining protection |
| `core/launch.rs` | Leases an output after selecting a binding | Existing targets can outlive app metadata; shim lifetime needs separate maintenance coordination |

Tests already cover managed launch, output retention, independent claims, interrupted record publication, and deactivation. The existing acceptance recipes exercise real release builds; the lifecycle recipe must extend that evidence across operations rather than merely collect command-specific tests. Main specs are empty; the completed changes remain unarchived.

## Goals / Non-Goals

**Goals:** One observable lifecycle, shared preparation/commit boundaries, explicit durable retries, exact ownership checks, and joint acceptance criteria. Keep `source/`, `index/`, `store/`, and `bin/` responsibilities intact.

**Non-Goals:** A graph, daemon, generic workflow engine, rollback of remote discovery/cache activity, universal filesystem power-loss transactions, or automatic deletion of cached outputs during uninstall. Scope exclusions are in proposal.md.

## Decisions

### 1. Reconcile operation meaning before implementing commands

| Operation | Claim effect | Binding effect | Shim effect | Output/source effect |
| --- | --- | --- | --- | --- |
| `register-app` | Empty claims | Inactive | None | None |
| `acquire --app`, `update-claim` | Create/move selected resource claim | None | None | Existing acquisition/reclaim rules |
| `install NAME REQUEST PROBE BUILT_SHIM --shim NAME` | Create initial managed claim | Select validated candidate | Publish owned shim | Publish/reuse candidate; source is independent |
| `update-app APP CLAIM PROBE` | Move selected claim after probe | Select same candidate in same commit | Unchanged | Publish/reuse candidate |
| `rollback APP CLAIM ENTRY PROBE` | Move selected claim to retained entry | Select same validated entry in same commit | Unchanged | No discovery or downloading |
| `activate APP ENTRY EXE PROBE` | None | Binding-only activation, including manual rollback | Unchanged | Existing integrity/lease rules |
| `deactivate APP` | None | Clear only binding | Unchanged | No reclaim |
| `uninstall APP` | Release all app claims at logical removal | Clear and reject new app operations | Remove owned files, possibly pending | No deletion; later reclaim applies |
| `install-shim`, `replace-shim`, `adopt-shim` | None | None | Explicit owned maintenance | None |
| `reconcile [--operation UUID]` | Only replay recorded intent | Only replay recorded intent | Resume safe recorded work | No implicit purge or arbitrary repair |

`bootstrap` and `self-update` become Saucepan-specific adapters over install/update, retaining github_release-only self-management and the Saucepan version probe. They do not duplicate the coordinator. Native APIs return the same typed results as CLI adapters.

Managed update/rollback require one declared executable, `useable_app`, and a host-compatible selection. Rollback requires the selected claim's item key to match the target and preserves its original request/policy; it does not rewrite a tracked selector into a historical tag. Arbitrary binding-only selection remains `activate`. Ordinary claims and active bindings remain independent outside the explicit managed commands.

This preserves resource acquisition as a useful API rather than silently turning every claim update into program activation. No new `enable` command or permanent disabled flag is introduced.

### 2. Prepare candidates without exposing app state, then commit once

Refactor acquisition to offer an internal prepared/leased candidate result. It may publish an immutable output and refresh discovery, but it must acquire the candidate's native lease under the publication lock before automatic reclaim can remove it. Keep that lease through full verification, bounded probe, post-probe verification and commit. Reuse follows the same lease handoff. Do not implement this as an unprotected `acquire(); lease()` sequence.

A durable commit-intent also protects its exact candidate against reclaim after the preparing process exits. Reclaim reports this temporary operation protection explicitly. Reconciliation reacquires a native lease and verifies candidate integrity before finishing activation; a missing/corrupt candidate fails the transition without changing an existing usable app. Completion or terminal failure releases operation protection. Uncommitted preparations carry no permanent pin after process exit and must reacquire/revalidate on retry.

For install, reserve an app UUID and claim UUID in operation intent without publishing a usable application. After validation, publish the complete owned launcher pair, then journal the full app record and operation completion together. Launches during initial partial publication fail closed because no usable app exists. A preparation failure leaves no live registration/claim/binding; cached output and refreshed source metadata may remain. A committed install intent resumes the same UUIDs rather than creating another app.

For managed update/rollback, capture a mutation revision and the selected claim; after probing, re-read under the mutation lock and commit the selected claim and binding in one app record. Add an app-local monotonic revision (default zero for existing records), advanced by all app mutations. A revision conflict fails with a retryable conflict before publishing stale state. Preserve unrelated fields from the current record; never overwrite an old full app snapshot. This chooses explicit retry over lost updates, including ABA changes.

The lower-level `activate` retains its established last-commit behavior: a probe already in flight can activate after `deactivate`. Managed update/rollback use the stronger conflict rule and can fail when deactivation changes their captured revision. Uninstall is different: a removing or removed app rejects either kind of activation/acquisition publication, preventing resurrection. Document this distinction, rather than silently changing the completed deactivation contract.

### 3. Durable operation records, with bounded recovery

Store domain-specific operation intents under `.staging/lifecycle/<operation-uuid>/index.json`; this directory is durable bookkeeping, not disposable preparation data. Record kind, app UUID, normalized input fingerprint, expected revision, reserved names, staged file digests, previous/desired state, phase, pending paths/reasons and result. Never store credentials in these receipts. Terminal receipts remain for idempotency; compaction is separate work.

Multi-step commands accept optional `--operation UUID`. Reusing it with identical inputs resumes or returns its prior result; changed inputs fail before effects. Generated IDs are included in results/errors, and `operations [--app APP]` discovers receipts after a lost response. UUIDs identify operations; names and timestamps do not identify retries. Two separate install requests with the same app display name are permitted only if their shim reservations do not collide.

Use narrow typed phases: preparing, commit-intent, cleanup-pending, complete, failed/conflict. Extend the existing short record journal to atomically express app state, operation phase/receipt and exact app-record deletion. Recovery finishes that bounded record transition before other cooperating operations observe it. File cleanup/replacement is handled by the operation coordinator, not by a global journal that makes every store open fail on a busy launcher.

Serialize execution of a given operation with a native operation lock acquired before shim/mutation coordination. Two callers cannot run the same intent simultaneously. A pre-commit terminal failure releases reservations once any verified staged/published files are accounted for; unresolved cleanup retains only the affected reservations and reports them. Retrying a terminal validation/conflict failure requires a new operation ID, while interrupted preparation and pending cleanup resume the original ID. No age-based assumption turns an operation into abandoned work.

`Store::open` continues bounded recovery of committed metadata; it does not download, run probes, wait for external processes, or sweep arbitrary directories. `reconcile` explicitly retries known filesystem work. Preparation without a commit intent is never silently considered validated: explicit retry rechecks candidate integrity and reruns its probe. After commit intent, recovery revalidates the staged identities it publishes. Corrupt/missing preparation is reported failed or pending with an actionable reason; an already usable app is not replaced with unverified files.

Operation JSON reports `operation`, `app`, `status`, `changed`, and pending paths/reasons plus operation-specific results. Completed success exits 0; recorded pending/conflict exits nonzero and retains the operation identifier. Preflight errors use ordinary errors, without success-shaped JSON. A completed retry returns the recorded result without another mutation. No claim of universal power-loss durability is added.

### 4. Explicit shim ownership and pair reservations

Add a typed shim ownership list to application metadata, default empty for existing apps. Each record contains executable name, derived sidecar name, exact executable digest, expected sidecar configuration and its digest. Keep the existing `{app, saucepan}` sidecar shape so launcher lookup stays small. Ownership is filesystem bookkeeping within the OS-user trust boundary, not authentication.

Reserve both executable and sidecar paths under mutation coordination before publication. Reject collisions against all app ownership and in-progress operation reservations, as well as occupied unowned filesystem paths. Compare portable case-folded names and their derived sidecar names, including `tool` versus `tool.exe` and names ending in `.shim.json`. Existing `relative_path`, `beneath`, and `plain_metadata` checks remain mandatory; links/reparse points and directories are never followed or recursively removed as launchers.

`install-shim APP NAME BUILT_SHIM` uses a durable install intent. `replace-shim APP NAME BUILT_SHIM` operates only on an owned pair and preserves app/mode configuration; sidecar repointing is not launcher replacement. Atomic executable replacement plus unchanged sidecar avoids a mismatched new target configuration. Keep the old working pair until the replacement is validated and ready. Missing owned files can be restored from explicit supplied input, but a present digest/config mismatch is a conflict, not permission to overwrite.

`adopt-shim APP NAME BUILT_SHIM` is explicit: require an exact digest match against the supplied trusted build, a matching existing sidecar and a live app. It registers ownership without replacing files. Unregistered legacy pairs are reported by inspection/reconciliation; uninstall does not guess ownership from filenames or sidecars. To proceed with uninstall when a recognizable unowned pair remains, the caller must adopt it or explicitly choose `--leave-unmanaged-shims`; the result identifies the files left behind. This prevents silently claiming a full removal.

Use a native per-shim maintenance lock: new shims retain shared ownership while reading configuration and, on Windows, while waiting for the target; maintenance takes exclusive ownership without long waiting. Launch acquires shim coordination before store mutation, maintenance follows the same order, and contention is reported pending. On Unix the shared lock need not survive successful exec. Do not wait for shim locks while holding mutation coordination. Old shim processes have no new lock: OS sharing errors remain pending, and adoption/maintenance documents that older writers must be stopped. Running target protection remains the output lease, independent of shim metadata.

### 5. Uninstall is a recoverable transition, not recursive root deletion

Under mutation coordination, journal the application into removing state with no active binding and no claims, preserving its exact owned-file inventory in the operation. All app-mutating entry points reject removing state. Existing targets and native resource guards continue independently. New shim launches reject the app's state even before physical launcher deletion is possible.

Delete only the owned regular executable and sidecar after rechecking recorded identity. Missing owned files count as already cleaned. Busy files remain pending. Changed/unrecognized files remain untouched and report a conflict. Once owned paths are accounted for, retire the app directory by a journaled same-filesystem rename into its operation directory, checking first that it contains only the expected removing `index.json`. Replay recognizes either location. This avoids leaving a UUID directory without an index, which would break the current index scanner. Remove the retired known file and empty directory individually; unknown additional files leave the app removing and report a conflict, never trigger recursive deletion. Retain the operation receipt as the removal history. A repeated uninstall for this removed UUID returns the recorded outcome; an arbitrary unknown UUID is an error.

No uninstall step deletes source definitions, Git working data, output indexes or payloads. Later reclaim drops the removed app's claim demand and active protection, but still considers every surviving output's persisted policy, other app claims/bindings, and leases. In particular, removing a high-retention claim does not necessarily lower effective retention while live output records still demand that count. The current maximum-demand rule is preserved and exposed in results/docs, not silently replaced.

Self-uninstall through the Windows Saucepan shim can reach logically removed/cleanup-pending because the wrapper is running. Finish with a directly invoked retained/new build and `reconcile --operation UUID` after that wrapper exits. Do not spawn an unbounded hidden cleanup daemon, kill the target, or claim full removal prematurely. The same direct-binary path is available after deactivating Saucepan itself.

### 6. Reconciliation with the completed changes

| Existing contract | Decision in this change |
| --- | --- |
| Reimplementation: stable shim, no activation on download | Preserve; install/update share validated commit orchestration |
| Reimplementation: failed activation preserves usable binding | Strengthen managed update/self-update to preserve the selected claim as well |
| Reimplementation: manual rollback through activate | Preserve; managed rollback additionally coordinates one claim |
| Efficient access: metadata-only routine launch, explicit full checks | Preserve; all activation candidates still get pre/post-probe scans |
| Efficient access: claims are not pins; consumers own leases | Preserve across update, deactivation and removal |
| Marker cleanup: only process launches create inherited markers | Preserve; operation preparation uses native guards, not process markers |
| Deactivation: binding-only, idempotent, later activation allowed | Preserve low-level semantics; document managed-operation revision conflicts |
| Mock acceptance: inspectable real managed installation | Preserve existing mock; extend acceptance in a unique simulated root |
| Main spec tree is empty | Keep these deltas additive; reconcile overlapping requirements at later explicit sync/archive |

### 7. Package structure and utility responsibilities

- `models/index`: app revision/state, shim ownership and lifecycle operation/result types. Keep source/output request models independent.
- `core/lifecycle.rs`: install/update/rollback/uninstall coordination and operation APIs; `core/shims.rs`: ownership checks, reservation, publication and maintenance. Split internal tests into adjacent files if needed rather than enlarge the CLI.
- `core/index`: short journal, operation persistence, exact deletion and replay; `core/activation.rs`: reusable candidate validation/probe and binding-only operations; `core/bootstrap.rs`: thin self-management adapters.
- `core/store`: candidate lease handoff and existing retention/access behavior; `core/launch.rs`: launcher lifetime and state checks; `cli`: parsing and typed result formatting only.
- Reuse `utils/{fs,path,lock,tree,process}` for atomic writes, containment, locks, digests and bounded probes. No new generic utility/library is presently justified: operation phases and ownership reconciliation are domain logic. Verify any changed portable primitive at its own boundary before integrating it.

### 8. Verification is one lifecycle acceptance gate

Add deterministic barriers and phase-failure injection at the coordinator boundaries. Cover managed update versus deactivation/uninstall/claim mutation, prepared-candidate reclaim, competing name reservations, and restart before/after each metadata/file publication point. Test CLI/native parity and pending-result exit codes.

Extend a real-build harness under `target/lifecycle-acceptance/<uuid>/root`, preserving `mock`. Exercise install A; update to B; failed update with unchanged claim/binding; rollback; deactivate/reactivate; launcher replacement; two apps sharing outputs; a live old target during uninstall; busy-file pending cleanup; direct-binary reconciliation; repeated removal and final ordinary reclaim. Record exact builds, operation IDs, claims, bindings, owned files and pending reasons. Snapshot mock digests before/after. The change is complete only when the combined flow and focused fault tests pass; individual command completion is insufficient.

## Risks / Trade-offs

- More metadata -> Keep operation types narrow and receipts separate from ingested content; defer compaction rather than invent another schema version.
- Mixed old/new writer processes -> Document an exclusive upgrade boundary; existing JSON decoding does not promise old writer interoperability. Tests use new core and controlled copied old shims only.
- Windows executable sharing -> Return pending cleanup and support explicit reconciliation from a direct binary; never depend on a process exiting on command.
- Conservative name/identity checks -> Some manually changed files need explicit adoption or operator resolution; preserving unknown files takes priority over pretending cleanup succeeded.
- Optimistic managed-operation conflicts -> Retry can require another probe; this avoids committing stale intent while keeping slow work outside the global lock.
- Concurrent/raw external readers -> Cooperating APIs get commit/recovery guarantees; directly reading files mid-transaction is not a transactional snapshot.

## Migration Plan

Existing new-format records default to ready state, revision zero and empty ownership; legacy saucepan stores remain unsupported. Do not automatically adopt old shims or mutate the inspectable mock. Test a copy of existing records/sidecars and explicit adoption. New writes use the new metadata; reverting to an older writer against that modified root is unsupported. Use an untouched root backup or a separate fresh root when testing binary rollback of the manager itself. No schema-version inheritance or production deployment is part of this change.

Implement in dependency-ordered, verified checkpoints within this single change. Checkpoints remain required by repository guidance; they do not divide the design into disconnected proposals. Exclude planning artifacts from implementation commits, and do not auto-archive the completed earlier changes.
