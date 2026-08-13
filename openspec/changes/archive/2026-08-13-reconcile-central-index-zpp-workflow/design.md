## Context

See `proposal.md` for motivation. The Rust CLI is the root public composition owner; `sdk/python` is a separate public composition owner that drives the built CLI. The central-index implementation and extensive Rust/Python tests already exist in commits `88b9170` and `3da65a1`, but legacy root Gherkin is flat and non-executable, the SDK feature root is not capability-owned, SDK bucket pin/refresh methods are missing, and Rust format/lint gates fail.

## Goals / Non-Goals

**Goals:**

- Establish one independently runnable Behave root per affected public capability.
- Exercise public behavior through the real built CLI or public Python SDK.
- Reuse existing unit and Rust integration matrices rather than duplicate them in Gherkin.
- Add the missing SDK bucket pin/refresh surface without changing existing call behavior.
- Preserve the two existing local commits and all current serialization, exit-code, source-precedence, and dependency contracts.

**Non-Goals:**

- Rewriting the central-index implementation or its Git/index data model.
- Adding a root Python package, an HTTP client, or another BDD dependency.
- Publishing, pushing, tagging, staging, or committing.
- Treating repository layout or implementation classes as product behavior.

## Decisions

### Reuse the SDK's locked Behave toolchain

Root capability features run with the Behave dependency already locked under `sdk/python`; no second Python project or dependency is introduced. Native Behave commands remain authoritative. Optional `zpp behave` coordination is not selected by this change, so no `zpp.behave.yaml` is required.

### Capability roots own only thin adapters

Every root owns `<capability>.feature`, `environment.py`, `support.py`, and `steps/bindings.py`. Root `environment.py` and `support.py` delegate reusable workspace, Git-fixture, process, and JSON-state operations to `features/support/`. SDK roots delegate to `sdk/python/features/support/`. Bindings translate phrases into support calls and contain no product policy.

### Keep case matrices below the public BDD layer

The retained Rust and Python unit/integration suites continue to own parsing variants, target-normalization cases, serialization compatibility, error-code matrices, path-safety cases, and cache invalidation matrices. Each capability feature keeps the smallest public scenario needed to prove composition through the real CLI or SDK.

### Additive SDK surface

`Workspace.bucket_add` gains an optional reference argument while remaining source-compatible, and `Workspace.bucket_refresh` is added. Both invalidate cached bucket state on success. No entity shape, exception mapping, or runtime dependency changes.

## Behavior Traceability

| Delta capability | Public Behave evidence | Retained non-BDD evidence |
|---|---|---|
| configuration-loading | Missing configuration maps to exit 3 | TOML variants, defaults, and source activation remain Rust unit cases; custom jq selection is proven by bucket-search public BDD |
| bucket-registry | Pinning records a commit and refresh advances it | Duplicate, rollback, unknown-target, local/file URL, and serialization matrices remain Rust tests |
| bucket-search | Matching jq filter returns an entry | Empty/no-match and jq launch/error variants remain focused Rust tests |
| central-index | Collision precedence and repeated stderr warning | Required-field, extra-field, unreachable, and compatibility matrices remain Rust tests; unreachable fallback is already a public Rust integration case |
| manifest-resolution-chain | Manifest-less install succeeds with provenance | Repository-first, exhaustion, and additive compatibility retain focused Rust integration coverage; exhaustion is also checked by installation-contract BDD |
| installation-contract | Chain exhaustion remains exit 1 | Complete error-category and multi-source precedence matrices remain Rust tests |
| github-targets | Equivalent spelling matches while provenance is preserved | Branch/tag/commit and case-sensitivity matrices remain Rust tests |
| sauce-uninstall | Managed checkout removal and local-path preservation | Escape rejection, missing checkout, and unknown-name matrices remain Rust tests |
| sauce-update | Update adopts the current index entry ref | Unknown/local/default/ref/custom-Git variants remain Rust tests |
| python-sdk | Bucket pin/refresh plus provenance/extra entity fields | Command, exception, caching, import, and vendorability matrices remain Python unit tests |

Every delta requirement is covered by the listed public scenario or by a concrete focused-test reason above; no proposal, task, documentation, configuration-only rule, or implementation detail is translated into Gherkin.

## Fix Set

- Add shared root and SDK Behave lifecycle support.
- Add capability-local environment, support entry point, and thin bindings for every shaped root.
- Add optional-ref bucket registration and bucket refresh to the public SDK with focused fail-first unit coverage.
- Apply the current Rust formatter and resolve the three Clippy findings without changing behavior.
- Update documentation only where the SDK surface or verification command changed.
- Preserve all existing functional tests and run every capability root independently.

## Risks / Trade-offs

- **Behave support duplicates some Rust fixture setup** → Keep it limited to public-process orchestration and reuse generic Git/JSON helpers across capability roots.
- **Many small feature roots increase command count** → Prefer deterministic targeted execution; coordination remains optional until repository evidence justifies it.
- **Formatting produces a broad mechanical diff** → Isolate it from behavioral reasoning in review and verify no semantic changes with the complete suite.
- **SDK cache invalidation could drift from existing bucket mutations** → Route both new operations through the existing `_mutate` seam and retain focused unit assertions.

## Migration Plan

Replace the legacy flat Gherkin with capability-owned roots, add bindings and the SDK methods, then run focused RED/GREEN checks followed by the complete verification and package-build gates. If reconciliation is abandoned before archive, remove only this new change and the new verification/SDK edits; the preserved `0.3.0` commits remain intact.

## Workflow Checkpoint

- `clarify`: completed — the owner confirmed index-sourced update, repeated collision-warning, bucket-registry, and automatic SDK-surface policies; no unresolved outcome-changing decision remains.
- `shape`: completed — all active delta behavior is traced to a capability-owned public feature or a concrete focused-test reason, and every shaped feature parses independently.
- `plan-utilities`: completed — signature-level boundaries were limited to the additive SDK bucket commands and composition-owned Behave lifecycle support.
- `mature-utilities`: completed — focused SDK and support-contract tests were observed RED for the missing seams, then passed independently after the minimum implementation; the disposable utility gate was removed.
- `wire`: completed — every approved capability is bound through the real CLI or public SDK with thin local adapters; independent BDD exposed and verified the detached-checkout update correction.
- `form-specs`: completed — all ten accepted deltas were merged without removing unaffected requirements, and all canonical specs plus the active change pass strict validation.
- `finalize`: completed — locked metadata and dependencies, Python 3.9 syntax and tests, Rust format and strict Clippy, 126 Rust tests, all ten independent capability roots, vendorability, diff hygiene, both package backends, and strict OpenSpec validation passed. No Python lint/format tool is declared, no retained OpenLease or Agent Router successor cohort exists, and the completed change is archived. Commit, push, tag, and release authority remain withheld.
