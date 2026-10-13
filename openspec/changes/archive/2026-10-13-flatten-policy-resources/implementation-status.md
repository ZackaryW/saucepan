# Flat-policy implementation status — 2026-10-13

Tasks: 29/29 complete. Final verification passed. Active change artifacts remain
excluded from implementation commits. Neither active change has been archived.

Final checkpoint: `1312667` (flat-contract consumer acceptance). Only the two active
OpenSpec change directories remain untracked; implementation work is committed.

## Completed implementation checkpoints

- `efac840`: managed Request, installation/bootstrap and scoped acquisition accept
  reference-only intent. OutputPlan is derived evidence; claim inline overrides
  and missing-adoption execution fallback are removed.
- `b85b761`: canonical retrieval/acquisition inputs, private execution engines,
  portable catalog fixtures and examples. The final compiled inline adapter and
  second merger are deleted. Equivalent policy UUIDs share acquisition identity.
- `115ff98`: remove global/native refresh/stale bypass flags; use referenced leaf
  policies. Normalize native path conveniences through the resolver, retain both
  stages' verified evidence for replay and require complete output evidence.
  Missing lifecycle/source fields and authenticated mixed metadata are rejected.

Earlier checkpoints introduced catalog registration, exact lookup, immutable
UUIDs, revision-checked adoption, source visibility publication, authority fences,
claim-pinned choices and optional credential support. All are retained.

## Acceptance and documentation

The fresh-root fixture in `tests/policy_acceptance.rs` passed the combined local
Git/two-scope flow: equivalent output sharing, explicit one-scope adoption,
unchanged/changed subpath updates, latest-two revision retention, guarded copies,
reclaim, exact recovery, release and root-free no-store delivery with HOME and
USERPROFILE removed. No Power Eagle checkout is modified.

Scoped caller-owned receipt reopening uses canonical references and portable
catalogs without persistent effects. Persisted claim/registry policy_refs and
retrieval/selection/effective evidence no longer fill missing fields. Public
optional request fields retain their documented defaults. Focused contract,
policy CLI and composition checks pass after updating incomplete test builders.

Fourteen real CLI operations exercised README, claim CLI and policy lookup guide
payloads in isolated roots. Earlier reports are marked historical and live guides
use the reference-only contract. A retained 2026-10-10 binary refused the flat
root with exit 1 and no stdout, leaving all root files unchanged.

The initial broad run exposed one incomplete claim fixture; a subsequent run
encountered the old retrieval-plan fixture after strict evidence validation.
Both fixture failures have been corrected and focused retests passed. The final
full suite passed 264 top-level tests with zero failures and five optional native
keyring tests ignored across 47 targets. Log: `%TEMP%/saucepan-flat-complete-suite.log`.
Fmt and all-target Clippy passed; strict OpenSpec validation passed all 20 items.
All 65 local documentation-link targets exist. Windows x86_64, Rust 1.98.1 only.

## Boundaries and final evidence

No implementation tasks remain. See `docs/policy-verification.md` for the
combined contract and validation coverage. Crate inclusion/defer reasons remain in
`docs/composition-utilities.md`: consolidated sha2, existing hex; no walkdir,
dunce or generic graph/IAM engine without equivalence and benefit evidence.

Do not migrate historical mock/user roots, edit Power Eagle, push, run hosted CI,
or sync/archive in this apply task. Native OS-keyring checks, macOS/Linux and MSRV
runtime checks were not run. Eventual synchronization/archive order is
`compose-retrieval-and-acquisition` first, then `flatten-policy-resources`.
