# Proposal

## Why

Every path lookup currently hashes the entire stored output, and shim launch repeats that validation before obtaining its execution lease. General consumers receive a bare path that reclaim may invalidate while they are using it; the new source/output/claim model needs efficient lookup and an explicit resource lifetime API.

## What Changes

- **BREAKING behavior:** make ordinary `path` resolution perform metadata and containment checks without reading or hashing content files. Add explicit `metadata` and `full` verification modes and a dedicated full-verification command, with truthful reporting of what was checked.
- Preserve full integrity validation for acquisition publication, reuse, recovery, activation, and rollback. Routine managed launch uses lightweight resolution with a lifetime lease rather than repeated full-tree hashing.
- Add a general `ResourceLease` API that returns an exact output path together with native reclaim protection. Support resolving an application's current claim and acquiring its lease as one coordinated operation.
- Add `run ENTRY_UUID -- PROGRAM ARG...` for external consumers. The child receives the resource directory through an environment variable and retains lifetime protection even if the wrapper exits unexpectedly.
- Reuse the existing native lock/marker mechanism across resource access, shim launch, and reclaim; perform optional full hashing under an entry lease rather than holding the global mutation lock throughout the scan.
- Verify the distinction between address lookup, content integrity, claims, and live use through public API, process, reclaim, and isolated mock acceptance tests.

## Capabilities

### New Capabilities

- `content-verification`: explicit access-time verification modes, mandatory integrity boundaries, truthful results, and no payload hashing during lightweight access.
- `resource-access`: exact-entry resolution, general consumer leases, claim-to-lease coordination, and a CLI process workflow protected against reclaim.

### Modified Capabilities

None in the main spec tree. `openspec list --specs` currently has no published capabilities. This change builds on the completed, unarchived `reimplement-saucepan` change and adds access contracts without weakening its output publication, reuse, retention, or activation requirements. Existing delta specs are not edited or silently archived.

## Impact

Affected areas: `src/models` access/result types, `src/core/store`, `src/core/launch.rs`, `src/core/activation.rs`, `src/core/index/claims.rs`, `src/cli/mod.rs`, existing path/tree/lock/process utilities, tests, README, and mock acceptance recipes. No new source provider, storage layout, persisted acquisition policy, or dependency is expected. Access mode is a per-call choice, not an output identity or retention-policy field.

Existing new-format stores remain usable. Scripts relying on bare `path` to detect payload tampering must request full verification. SDK ports, mirrors/snapshot commands, encrypted indexes, authenticated views, legacy migration, background verification, permanent pins, and hostile same-user isolation are outside this change. Only planning artifacts are created by this proposal.
