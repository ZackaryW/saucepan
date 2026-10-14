# Archived changes

These dated directories preserve completed OpenSpec changes and their original
planning, decisions and verification evidence. Later changes may supersede their
schemas, CLI examples and statements about what remains active or unimplemented.
Do not merge every historical requirement into a new runtime contract.

Use [current main specs](../../specs/README.md) for the reconciled requirements
and the [repository README](../../../README.md) for current usage. Dated host
verification belongs to the owning change below. Archived commands and acceptance scripts are checkpoint
evidence, not instructions to migrate or reopen an older store.

All changes present at the 2026-10-10 archive checkpoint were completed and
archived. Inspect active changes with `openspec list`; an archive's own status
text is historical.

## Supporting evidence

Reports formerly kept in `docs/` are colocated with their changes. Relocation
preserves the original dates and limitations; it does not rerun checks or update
the historical contracts.

| Change | Evidence and decisions |
| --- | --- |
| Reimplementation | [Simulated installation](2026-10-09-reimplement-saucepan/acceptance.md) |
| Resource access and subsequent cleanup | [Detailed evidence](2026-10-09-efficient-resource-access/evidence.md) |
| Application lifecycle | [Verification](2026-10-09-reconcile-application-lifecycle/verification.md), [release packaging checkpoint](2026-10-09-reconcile-application-lifecycle/release-verification.md) |
| Searchable claim context | [Verification](2026-10-09-searchable-claim-context/verification.md) |
| Claim registry | [Verification](2026-10-10-secure-application-claim-registry/verification.md), [integrity implementation notes](2026-10-10-secure-application-claim-registry/integrity-notes.md) |
| Optional keyring | [Verification](2026-10-13-make-keyring-protection-optional/verification.md) |
| Retrieval and acquisition | [Verification](2026-10-13-compose-retrieval-and-acquisition/verification.md) |
| Flat policies | [Verification](2026-10-13-flatten-policy-resources/verification.md), [utility and crate decisions](2026-10-13-flatten-policy-resources/utilities.md) |
| Group leases | [Verification](2026-10-14-add-group-leases/verification.md) |
| Registered groups and pins | [Verification](2026-10-14-orchestrate-resource-groups/verification.md), [consumer assessment](2026-10-14-orchestrate-resource-groups/consumer-assessment.md) |
| Python and TypeScript SDKs | [Verification](2026-10-14-add-language-sdks/verification.md), [design and dependency decisions](2026-10-14-add-language-sdks/design.md) |
