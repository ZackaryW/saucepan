# Tasks

Each numbered group is a dependency-ordered checkpoint. Complete its behavior, focused verification, and documentation before dependent work. Utility contracts U1-U6 are defined in design.md; verify each required portable contract with domain-neutral fixtures before integrating its core caller. Report the checkpoint and proposed commit; create commits only when separately authorized. No production store or installed executable is a test target. The final inspectable acceptance root is `C:/Users/ZackaryWang/Documents/GitHub/saucepan-2/mock`; ordinary isolated unit/integration fixtures remain temporary.

## 1. New-format records and isolated persistence

- [x] 1.1 Create one Cargo library/CLI/shim package following the supplied models/{source,index,store,policy}, core/{source,index,store,policy}, cli, and utils boundaries; choose MSRV/dependency versions and review licenses/features, wire nested test modules into Cargo discovery, and verify all targets build without utility imports of domain modules.
- [x] 1.2 Implement U1 relative-path validation, U2 atomic file replacement/staged directory publication, and U3 bounded native lock guards; verify their domain-neutral real-filesystem/process tests cover traversal, collisions, failed writers, occupied destinations, contention, timeout, and lock release before integrating persistence.
- [x] 1.3 Define typed domain records and implement explicit-root initialization, direct lookup, and JSON persistence in models/core/index; verify unknown type/policy rejection, valid request fixtures, legacy-root preservation, and no fallback to the production root.
- [x] 1.4 Document package ownership, source/index/store layout, utility contract limitations, and new-format trust boundary with valid JSON examples; verify examples deserialize through the public API and documented initialization runs in a temporary root.

## 2. Immutable local outputs and publication recovery

- [x] 2.1 Complete U1 bounded tree-copy mechanics with explicit stable-source/private-destination contracts; verify generic nested tree, byte/count limits, metadata, links/reparse points, and source-error fixtures before adding source acquisition semantics.
- [x] 2.2 Implement local acquisition in core/source and immutable output assembly in core/store using U1/U2; verify changed inputs produce distinct versioned outputs, selections are correct, ingested index.json remains ordinary content, and published files remain unchanged.
- [x] 2.3 Implement publication coordination and journal recovery in core/index using the verified filesystem/lock helpers; verify interruptions before/after publication expose no partial ready outputs and recovery is idempotent.
- [x] 2.4 Implement reuse/new_entry policy evaluation in core/policy and integrate concurrent publication rechecking; verify compatible reuse, explicit duplicates, conflicting policies, and concurrent convergence through actual acquisition callers.
- [x] 2.5 Expose local acquire/inspect/path through the CLI with explicit-root support and document their JSON results; verify CLI/native parity and unscoped acquisition creates no synthetic app.

## 3. Remote discovery and release asset selection

- [x] 3.1 Implement U4 staged bounded ZIP extraction over the selected zip crate; verify domain-neutral malformed/traversal/link/collision/limit/selection fixtures and unchanged destinations on failure before provider integration.
- [x] 3.2 Implement selector-keyed discovery caching in core/source with caller-supplied time and configured HTTP agent; verify pre-24-hour hits, exact expiry, non-sliding reads, force refresh, backward clocks, and failed-check timestamps against a fixture transport.
- [x] 3.3 Implement github_release latest-stable/tag resolution and ordered platform/asset selection with direct globset use; verify matching, zero/ambiguous results, shared discovery across platforms, and no Git invocation.
- [x] 3.4 Integrate HTTP(S) asset/URL acquisition, raw/ZIP modes, and checksum verification with U4 and output publication; verify corruption and unsupported formats fail without claim/output changes, and valid payloads preserve their natural structure.
- [x] 3.5 Document remote source payloads, binary selection, explicit fixture endpoint configuration, freshness, and stale fallback; execute examples through the CLI against a deterministic local HTTP server, including failed discovery/recovery.

## 4. Reusable repository sources

- [x] 4.1 Implement U6 literal-argument, bounded-capture process execution with deadlines and child cleanup; verify real fixture binaries exercise simultaneous large stdout/stderr, nonzero exits, timeouts, process trees, and paths/arguments with spaces before Git integration.
- [x] 4.2 Implement U5 fetch/resolve/export in the existing utils/source/git.rs location using Git CLI and U1/U2/U6, without domain model imports; verify local repositories, origin mismatch, exact commits, submodules/LFS, link materialization, executable metadata, failure cleanup, and timeouts through portable contracts.
- [x] 4.3 Implement core/source Git/github_repo normalization, source working-data ownership, and policy-controlled refresh using U5; verify aliases, tracked/pinned requests, and independent unchanged prior outputs through core acquisition tests.
- [x] 4.4 Document Git utility effects/tool requirements and repository CLI behavior; execute a two-revision fixture demonstrating shared source data, separate immutable outputs, and request provenance.

## 5. Application claims and independent updates

- [x] 5.1 Implement app registration/settings and request-bearing claims with claim IDs; verify multiple selections from one source remain distinct and failed acquisition leaves existing claims unchanged.
- [x] 5.2 Implement claim update/release and claim-scoped filtered views; verify app B can update without moving app A, release does not remove another app's association, and listing never creates claims or acquisition permissions.
- [x] 5.3 Extend recoverable publication to claim changes and expose the claim lifecycle through CLI commands; verify failure-injected updates preserve a valid old or new claim and documented two-app examples execute against isolated fixtures.

## 6. Item policies and automatic output reclaim

- [x] 6.1 Implement item-key retention groups, creation-order ranking, latest-ten default, acquisition overrides, and deterministic effective retention across differing policy counts; verify the eleventh ordinary entry reclaims the oldest, reuse does not reorder history, and separate platform/folder histories remain independent.
- [x] 6.2 Implement ready/reclaim_pending/reclaimed transitions and retryable deletion retaining provenance; verify claimed ordinary entries can be reclaimed, sources remain intact, partial deletion is reported honestly, and path lookup rejects unavailable entries.
- [x] 6.3 Compose U3 into domain execution leases and reclaim exclusion in core/store and core/launch; verify concurrent launch/reclaim, target process exit, and unexpected launcher exit cannot release protection while the target still executes, and eventual cleanup uses native lifetime evidence rather than PID-only ownership.
- [x] 6.4 Integrate automatic policy evaluation after successful acquisition plus explicit reclaim/reacquire commands; document effective policies and exceptions, and verify a claimed reclaimed item can be acquired again without reviving a misleading old entry identity.

## 7. useable_app activation and stable shims

- [x] 7.1 Implement declared executable bindings in models/index, probe orchestration in core/activation using verified U6, and useable_app eligibility in core/policy; verify invalid platform/path/probe results preserve the old binding and ordinary claims still permit reclaim.
- [x] 7.2 Implement atomic activation/rollback integrated with the journal and leases; verify interrupted switches recover consistently, unavailable rollback fails clearly, and old running targets remain present until leases end.
- [x] 7.3 Build the separate shim entrypoint using core/launch and the narrow index binding API; verify argument/stream/status/interruption forwarding, explicit-root propagation into child Saucepan, no recursive target lookup, concurrent activation, and lease lifetime with actual child processes.
- [x] 7.4 Document generic managed-app setup and protection exceptions; verify a two-version fixture keeps launching the old target until activation and the new target afterward without choosing by download timestamp.

## 8. Saucepan bootstrap and integrated acceptance

- [x] 8.1 Implement Saucepan bootstrap/self-update through github_release acquisition and useable_app using the separate built shim; verify integration fixtures activate replacements without Git cloning or overwriting a running application.
- [x] 8.2 Add a repeatable acceptance harness/fixture recipes outside mock, plan generated mock contents into gitignore, and document explicit-root use and rerun behavior; verify the harness inspects existing state and never recursively wipes the mock root or accesses the production store implicitly.
- [x] 8.3 After implementation, build the actual Saucepan and shim in release mode, package the application as a release asset, and acquire it through the real provider against a loopback GitHub-compatible fixture into C:/Users/ZackaryWang/Documents/GitHub/saucepan-2/mock; verify source/index/store records, downloaded build digest, app claim, useable_app activation, and actual launch from mock/bin with the same explicit root, leaving the resulting installation inspectable.
- [x] 8.4 Run the complete simulated update with a second distinguishable real application build, failed candidate, old running process, rollback, two-app claims, controlled 24-hour expiry, duplicate policies, eleven-entry reclaim, and reclaimed-item reacquisition; record build identities, entry UUIDs, active bindings, process results, and root isolation rather than treating relabeled identical bytes as a version change.
- [x] 8.5 Document the demonstrated mock layout, bootstrap/update/rollback commands, utility/package boundaries, launcher maintenance, and unsupported legacy protocols; execute the guide and record actual Windows/Linux/macOS evidence or unavailable platform execution explicitly.
- [x] 8.6 Run final formatting, lint, all-target build, and relevant full-suite checks once integration is complete; verify each spec scenario has evidence, review the diff for unrelated changes, and report remaining platform limitations before declaring implementation complete.
