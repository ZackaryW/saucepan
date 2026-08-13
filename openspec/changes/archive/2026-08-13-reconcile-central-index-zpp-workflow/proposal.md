## Why

The unreleased central-index manifest-chain work is functionally implemented, but it predates the repository's current ZPP workflow: several public behaviors are not traced through capability-owned Behave roots, some existing feature contracts lack canonical capability ownership, explicit stage outcomes are absent, and the final format/lint gates are not green. This change reconciles that repository evidence without rewriting the two existing local commits or changing established runtime behavior.

## What Changes

- Ratify the current central-index and manifest-resolution behavior as the compatibility baseline for the unreleased `0.3.0` work.
- Make two previously underspecified public policies explicit: updating an index-sourced sauce re-reads the registered index and may adopt its current entry ref; shadowed index entries warn on stderr during every resolution.
- Give the repository's established configuration, bucket-registry, bucket-search, and sauce-update behaviors canonical capability ownership so their retained Gherkin can be traced rather than treated as orphaned evidence.
- Reconcile the affected existing installation, GitHub-target, uninstall, and Python SDK contracts with capability-owned public Behave coverage while keeping pure input and serialization matrices in unit tests.
- Introduce no new runtime dependency and preserve the standard-library-only Python SDK boundary.
- Repair the Rust format and lint gates, run the complete declared verification and package-build set, record all ZPP stage outcomes, and archive this reconciliation change through the current workflow.
- Preserve commits `88b9170` and `3da65a1`; pushing, tagging `v0.3.0`, and publishing binaries remain separately authorized release actions.

## Capabilities

### New Capabilities

- `configuration-loading`: Existing public configuration loading, defaulting, validation, and command-facing overrides.
- `bucket-registry`: Existing bucket/index registration, duplicate rejection, removal, listing, pinning, and refresh behavior.
- `bucket-search`: Existing jq-backed search behavior over registered bucket entries.
- `sauce-update`: Existing update behavior, including index re-resolution, ref handling, index refresh, and stable failures.

### Modified Capabilities

- `central-index`: Make collision reporting on stderr during every resolution explicit and trace registry/pinning behavior through the public boundary.
- `manifest-resolution-chain`: Make update-time re-resolution against the current registered index explicit and trace ordered resolution through the public boundary.
- `installation-contract`: Reconcile retained install and failure behavior with the capability-owned executable contract.
- `github-targets`: Reconcile target normalization and ref-sensitive install behavior with the capability-owned executable contract.
- `sauce-uninstall`: Reconcile the retained public uninstall contract with capability-owned executable coverage.
- `python-sdk`: Reconcile public SDK lifecycle, manifest provenance, and bucket-extra behavior with an independently runnable SDK-owned feature root.

## Impact

- Planning authority: `openspec/changes/reconcile-central-index-zpp-workflow/` and the listed canonical capabilities.
- Behavior verification: root `features/<capability>/` surfaces and `sdk/python/features/<capability>/`, with reusable lifecycle support under each composition owner's `features/support/`.
- Implementation: existing Rust CLI and Python SDK behavior is preserved unless verification exposes a contradiction; expected source edits are limited to format/lint repairs and any smallest coherent correction proven by a relevant RED.
- Dependencies: reuse the repository's existing Rust toolchain, Python lock, Behave dependency, Git fixtures, and standard library before considering new code or packages.

## Confirmed Decisions

- Central indexes continue to use the existing bucket registry.
- Updating an index-sourced sauce re-reads the registered index and may adopt the entry's current ref.
- A later registered index that also describes the target emits a warning on stderr during every resolution; the earlier registered index remains the winner.
- The Python SDK retains its complete CLI-surface promise by adding support for `bucket add --ref` and `bucket refresh`.
- Existing successful operations, serialized compatibility, exit-code categories, source precedence, and dependency boundaries remain unchanged.

## Unresolved — Do Not Assume

None.

## Workflow Checkpoint

- `clarify`: completed — the proposal and complete capability-delta contract validate with no unresolved product decision.
