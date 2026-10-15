# Implementation evidence — 2026-10-15

## Delivered checkpoints

- `c11313c`: pure immutable profile expansion, CLI previews, overrides and user guide.
- `2df06fe`: authorized effective-policy explanations, composition/freshness/retention checks.
- `8ae3695`: profile-assisted managed setup and actual-build acceptance.
- `2e47da0`: equivalent Python/TypeScript profile and lifecycle adapters, explicit receipt decoding and contract fixtures.
- `d05f1ea`: explanation refuses pending metadata recovery without journal/catalog writes (regression observed RED, then GREEN for native and CLI).
- `1356e8a`: installed SDK lifecycle acceptance and optional checking-workflow wiring.

Active change files remain excluded from these local zmem-authored commits. No push, release, hosted workflow dispatch or consumer repository migration was performed.

## Native behavior

Focused profile (6), explanation (6), existing policy CLI (7), adoption (3), claims (3), composition (11), pins (9) and lifecycle (16) tests passed on Windows. Profile expansion does not require home/root; registration remains explicit; same policy UUIDs have fixed bodies; request overrides cannot weaken registered constraints; pinned old outputs survive outside the latest-ten history window. Explanation uses the metadata-only boundary and returns `recovery_required` for pending transactions without replaying them.

`scripts/lifecycle_acceptance.py` completed 32 operations with two actual release builds (`lifecycle-a`, `lifecycle-b`) and distinct binary SHA-256 values. Evidence is in `target/lifecycle-acceptance/ffe402ce-5f33-482d-8daf-0f7d9f5a2dbf/report.json`. Bootstrap, shim launch, update, failed replacement, rollback, uninstall, pending Windows shim replacement/reconciliation, shared-owner protection and reclaim passed. Historical `mock/` content was unchanged.

## SDK and installed-package behavior

- Python: 19 tests passed; strict mypy passed for 19 source/example files.
- TypeScript: 12 tests and declaration build passed.
- Both shared real-manager suites exercised all 57 declared operations. Profiles/explanations were root-free; loopback GitHub releases exercised bootstrap/self-update. Bound administration denial did not fall back to local access.
- Cross-language consumer scenarios: 3 tests passed in isolated fixtures; this does not assert migration of external applications.
- Complete and noncomplete (preparing, commit-intent, cleanup-pending, failed, conflict) receipt decoding, mixed reconciliation, read-only inspection, domain errors, malformed receipts and indeterminate mutation handling passed in both transports.
- Built wheel, sdist and npm tarball were installed outside the checkout. With PATH emptied and explicit release manager/shim paths, all completed profile/explanation, install, update, failed-probe preservation, rollback, operation inspection, explicit reconciliation and uninstall checks. The failed operation remained exit 2 with status `failed`; successful operations remained exit 0. Expansion digest matched across languages: `8b54dceab7e35da886b3b91b35572d037633560ace143940244e5a168f8202a9`.
- Python local-folder, editable and exact-commit Git/subdirectory installs also passed that flow. The Git test used an isolated local repository, with no pushes or changes to this repository's refs.
- Public installed TypeScript declarations passed positive and negative consumer compilation checks. Runnable profile and managed-setup examples passed in isolated roots.

## Environment and boundaries

Observed host: Windows, Rust 1.98.1, Python 3.14.6, Node 26.10.0. Linux/macOS execution, minimum-version runtimes and native OS keyring custody were not exercised here. Native-keyring checks remain explicitly selected; basic operation and injected-key tests do not establish native custody.

No new dependencies, SDK-side recipes, schema compatibility facades, automatic retries, binary provisioning or PATH changes were introduced. SDK checking workflows now build the matching shim and select installed lifecycle verification; release workflows were untouched.

## Final checks

`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, SDK contract inventory and strict OpenSpec validation passed. `cargo test --locked --all-targets -- --test-threads=1` passed: **332 passed, 0 failed, 5 intentionally ignored native-keyring integrations**. The log is `target/policy-profiles-rust-tests.log`, including the final pending-recovery explanation regression. All six focused explanation tests also passed after that correction.

The first parallel all-target run stalled in the unchanged `changed_policy_has_independent_proof_and_failed_materialization_does_not_advance_it` acquisition test. The same test passed alone in 7.41 seconds and in the completed serial suite. The stalled test process was stopped after several minutes; its run is not a passing result (`target/policy-profiles-rust-parallel.log`). The cause of the parallel-only stall is not established, and no test/workflow concurrency settings were changed to conceal it.

## Release preparation and consumer handoff

Saucepan's package and lockfile version is prepared as `0.10.0`; both language SDKs and their lockfiles are prepared as `0.2.0`. The feature release exposes the profile and managed lifecycle contracts without changing existing adopted policy identities. Release examples use the new versions. No tag or release was created.

After the metadata-only version bump, `cargo check --locked --bins`, Python lock validation, all 12 release packaging tests and strict OpenSpec validation passed. The Python wheel/sdist and TypeScript tarball were rebuilt at `0.2.0`; release validators verified their metadata and contents against current sources, and accepted the intended `v0.10.0` tag value. These local checks do not establish published-release availability or consumer integration.

All five delta capabilities were synchronized to main specs, preserving pre-existing scenarios; all 18 tasks were complete at archive. The release-preparation and archive changes are left for the user's commit. Only after the user confirms that commit should separate agents adapt power-eagle, powerspec-2 and zush-2. Published-artifact verification must use the new release's actual manager and SDK packages once available, recording its tag, commit and artifact identities rather than treating local builds as release acceptance. No consumer work, commits or pushes were authorized by this preparation step.
