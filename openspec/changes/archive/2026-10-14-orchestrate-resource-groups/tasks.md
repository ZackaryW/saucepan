# Tasks

## 1. Durable output pins checkpoint

- [x] 1.1 Add models/pin and core/pin contracts, verified catalog identities and pin_policy with distinct pin/unpin actions; verify strict bounded payloads, owner-local keys, immutable targets, explicit registered policy roles and no implicit source/app/group inheritance.
- [x] 1.2 Implement atomic pin admission, owner-scoped inspection/release and reclaim exclusion using existing coordination and metadata publication; verify multiple independent pins, duplicate retries, released-key conflicts, history-none retention, restart persistence, corruption refusal, admission/reclaim races and partial verification failure cleanup.
- [x] 1.3 Expose native and CLI pin-outputs, pin, pins and unpin operations; verify current app credentials/source/scope rules, no local fallback, inspection after claim movement, denied new retention versus permitted cleanup, missing payload cleanup, orphaned pins after uninstall and basic/injected-key modes.
- [x] 1.4 Document the pin A / pin B / switch / unpin A workflow with parsed payload examples and a process-exit retention fixture; verify affected reclaim/security/lifecycle suites and commit the coherent pin slice using zmem, excluding active change artifacts.

## 2. Verified group registration checkpoint

- [x] 2.1 Add strict group request/record models under models/group, reusing existing selectors, selections, PolicyRefs and name limits; verify valid local/app examples and rejection of unknown fields, duplicate names, unsafe views, mixed owners and over-limit inputs.
- [x] 2.2 Add catalog group identity, path validation and registry operations under core/group; verify concurrent idempotent registration, divergent-key conflicts, UUID/key lookup, expected-revision replacement/removal, basic/injected-key integrity and app retirement without deleting claims or output content.
- [x] 2.3 Integrate registered-only policy references, per-member authority and group restrictions for preparation actions; verify denied app/local boundaries have no provider effects, lease-only access cannot fetch, scope witnesses cannot be invented and invalid cache/destination combinations fail preflight.
- [x] 2.4 Document registration, ownership, lookup and the managed-output bound with parsed JSON fixtures; run focused model/registry/policy tests and commit this coherent slice using zmem, excluding active change artifacts.

## 3. Deferred publication and preparation checkpoint

- [x] 3.1 Split existing acquisition preparation from selected-check/claim publication into an internal candidate result without introducing a second public payload shape; verify current single-resource regression tests and failure cases leave claims and successful-check timestamps untouched while complete candidates remain reclaimable.
- [x] 3.2 Implement complete-set claim matching and deterministic intent coordination with one waiting budget; verify duplicate/covered fresh members, order independence, distinct scopes, rejection of broadening existing narrower claims, context/policy conflicts and timeout cleanup using isolated concurrent fixtures.
- [x] 3.3 Implement policy-driven candidate preparation and compatible shared Git retrieval; verify initial acquisition, manual/due updates, all reclaim strategies, permitted stale evidence, fixed-output unavailability, common commit for changed subpaths and no self-contention across incompatible source requests.
- [x] 3.4 Document candidate side effects and freshness/revision distinctions, add callable internal preparation tests and run affected retrieval/acquisition/claim suites; commit the verified refactor and preparation slice with active artifacts excluded.

## 4. Atomic generation publication checkpoint

- [x] 4.1 Publish group selections, proposed acquisition checks and affected claims with one metadata transaction and one app revision increment; verify no partial visibility, preserved unrelated claims/context/bindings, no-op behavior and exact generation provenance.
- [x] 4.2 Fence group/app/claim/policy observations through provider work and protect candidates through commit; verify races with replacement, removal, claim release/update/adoption, credential changes, uninstall and reclaim without stale resurrection or leaked guards.
- [x] 4.3 Extend crash fixtures across preparation, durable journal, record writes and catalog publication; verify recovery yields one complete selected generation and that indeterminate post-commit errors are reconciled by revision lookup rather than blindly repeating new_entry operations.
- [x] 4.4 Document the metadata-only atomicity boundary and retained candidate cleanup; run focused publication/recovery tests and commit this verified slice with active artifacts excluded.

- [x] 4.5 Integrate explicit prepare-and-pin into final generation publication and add standalone pin-group at an expected revision; verify continuous protection through commit, all-or-nothing claims/checks/generation/pin recovery, idempotent lost-response retries, policy fences, stale group conflicts and preservation of older independent pins. Document these boundaries and checkpoint the verified integration with active artifacts excluded.

## 5. Retained native and CLI workflows checkpoint

- [x] 5.1 Implement native read-only opening and prepare-and-open of registered generations using existing group guards; verify stale claim witnesses fail, generation replacement cannot redirect existing consumers and prepare-to-lease handoff has continuous protection.
- [x] 5.2 Add register/show/list/replace/remove/prepare/run-registered/prepare-run CLI commands using existing root/app/error conventions and bounded inputs; verify UUID/key lookup, expected revisions, no partial success stdout, member diagnostics, literal child arguments/status and unchanged run-group behavior.
- [x] 5.3 Add documented executable local and app-owned examples, including a command/hooks/service-shaped consumer; verify child spawn failure, wrapper termination, supported handle retention and removal during use with isolated process fixtures. Explain that detached service supervision and unsupported handle-closing runtimes need consumer integration.
- [x] 5.4 Run focused CLI/lifetime/documentation contract checks and commit this integrated slice using zmem, excluding active artifacts.

- [x] 5.5 Expose CLI preparation pin options and update README, pin/group guides and consumer-integration-assessment.md to describe direct pinned store access for Power Eagle and invocation leases for Zush; execute documented local/app examples, reject wrong-role pin policies and verify no copy/sync/hook subsystem or compatibility adapter was introduced. Commit the verified docs/CLI integration with active artifacts excluded.

## 6. Whole-change acceptance checkpoint

- [x] 6.1 Run cargo fmt --all -- --check, cargo clippy --locked --all-targets -- -D warnings and cargo test --locked --all-targets once after integration, plus strict OpenSpec validation; record exact platform and optional keyring exclusions without triggering hosted workflows.
- [x] 6.2 Execute fresh isolated Power Eagle-shaped plugin and Zush-shaped multi-resource fixtures covering prepare-and-pin, updater process exit, delayed reads from pinned A while B activates, independent consumer pins, explicit unpin, repeat prepare, upstream change, partial failure, stale generation and live-consumer reclaim; preserve mock and external repositories, report fixture evidence separately from real app integration and commit the acceptance report with active artifacts excluded.
