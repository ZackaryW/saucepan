# Design

## Context

See proposal.md. Applications already persist `active: Option<Binding>`. Activation, claim updates, shim selection and reclaim use mutation coordination; binding commits use the existing recoverable journal. No new storage type or utility is necessary.

## Goals / Non-Goals

**Goals:** Clear one binding with the same persistence guarantees as activation, without interfering with independently owned claims or leases.

**Non-Goals:** Uninstall, process termination, automatic reclaim, permanent disabling, new activation cancellation semantics, or installed shim changes.

## Decisions

- `Store::deactivate(app) -> Result<Option<Binding>>` reads the application under the mutation lock, takes its active binding and journals the updated app only when there was a binding. Returning the removed binding distinguishes a change from an idempotent no-op. Unknown UUIDs and wrong record kinds fail before writes.
- CLI `deactivate APP_UUID` requires the existing explicit root. Return `{app, deactivated, previous_binding}`; `deactivated` reports whether this invocation removed a binding. Repetition returns false and null. It does not assert that no later concurrent activation has occurred.
- Read the current app record while locked so claims/settings already committed by other operations are preserved. Launch and deactivate serialize at binding selection: launch that already acquired its lease remains valid; launch selecting after deactivation fails with the existing no-active-binding error. A later activation commit, including an already-running probe, may reactivate the app. Deactivation is a binding mutation, not a persistent disabled flag.
- Do not validate the old output payload to clear its binding. A missing or corrupted payload must not prevent deactivation. Normal journal recovery remains required before the operation.
- Do not invoke reclaim here. Clearing the binding removes this app's active protection; later reclaim still considers other bindings, keep counts and live leases. Shims remain installed and can work again after explicit reactivation.

## Risks / Trade-offs

- Users expect immediate deletion: document the separate reclaim step and independent retention/lifetime protection.
- Interrupted binding write: reuse the journal and verify recovery before and after app-record publication.
- Concurrent activation can restore a binding: document commit-order behavior rather than silently introducing cancellation or a new disabled state.

## Migration Plan

No migration: deactivation writes the existing null active field. Test only temporary roots. One coherent implementation checkpoint includes API, CLI, documentation and focused tests; complete broader verification and commit without planning artifacts.
