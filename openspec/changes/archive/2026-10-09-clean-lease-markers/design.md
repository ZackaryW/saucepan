# Design

## Context

See proposal.md for the reproduced growth. `resolve` currently constructs a `ResourceLease`; `metadata_lease_locked` creates both shared entry coordination and a uniquely named native marker. Only `has_live_execution`, reached for evictable entries, removes closed markers. Existing process fixtures already test wrapper death and native guard non-inheritance.

## Goals / Non-Goals

**Goals:** Prevent non-process marker creation; clean abandoned markers for retained outputs; preserve launch/cleanup coordination and existing marker compatibility.

**Non-Goals:** Changing retention or claims, deleting stable coordination locks, replacing installed shims, timers, PID heuristics, repair or uninstall operations.

## Decisions

- Metadata resolution selects and validates under mutation coordination and returns directly. Full resolution and native guards still retain shared entry coordination across inspection/use. Store results and public API remain unchanged.
- A guard keeps a marker path and optional native marker handle. Only the private dedicated launch boundary creates, locks and marks that handle inheritable, while still holding shared entry coordination. There is no unprotected handoff. Normal native guards create no marker. Avoid unconditional unlink-on-drop: on Unix a child may still own the inherited lock on the unlinked inode, which would hide live ownership from reclaim.
- Reclaim sweeps recognized `live-<entry UUID>-<marker UUID>` files before applying retention. It attempts exclusive entry coordination without waiting, then checks native marker ownership. It leaves held markers, busy entries, malformed names and non-marker files untouched. It removes only closed regular marker files. Existing safety helpers reject links. Keep the fresh per-entry live-marker check at the content-deletion boundary; a cached sweep result cannot authorize deletion after a concurrent launch handoff.
- Reuse the current path, file-lock and platform marker mechanics. No new utility family or dependency is needed. Two coherent checkpoints separate marker-free access from stale-marker cleanup, with regression tests in each.

## Risks / Trade-offs

- Sweeping during marker creation could observe an unlocked file: exclusive entry coordination excludes creation while the launcher holds its shared lease.
- Wrapper exit is not target exit: native ownership checks preserve inherited markers, including older installed shims.
- A crashed process can leave a marker: the next reclaim/acquisition removes it once native handles close. No cleanup timing promise without an operation.
- Sweep work scales with marker count: ordinary reads no longer add markers, and a reclaim pass removes closed ones. Large-scale index optimization remains separate.

## Migration Plan

No persisted schema migration. Old launchers keep the same marker format and coordination lock. Their closed markers become sweepable. Test in isolated roots and copy an old shim into an isolated acceptance installation for compatibility; do not modify mock. Reverting binaries remains possible but restores accumulation.
