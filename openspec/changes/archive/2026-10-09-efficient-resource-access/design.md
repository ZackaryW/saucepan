# Design

## Context

See proposal.md for motivation. Read-only inspection found:

- `core/store/mod.rs`: `path` holds the mutation lock, reads an output record, and calls `verified_path`, which calls `manifest` and `utils/tree::inspect` to hash the entire tree. The same helper serves reuse and provider copy paths.
- `core/launch.rs`: public `execution_lease` already offers native protection, but carries no resolved path/result. `launch` calls `verified_path` and then `lease_locked`, which verifies again. Native markers use Windows file-sharing conflicts and Unix inherited shared locks; reclaim consults both coordination locks and these markers.
- `core/activation.rs`: initial validation and lease acquisition redundantly hash, followed by a distinct necessary post-probe verification before binding publication.
- `core/index/claims.rs`: claims carry exact entry UUIDs; listing and updates already coordinate with the mutation lock. There is no atomic claim-to-consumer-lease method.
- `cli/mod.rs` has `path` but no content-verification or general leased-run command. `utils/process` provides bounded capture/probes; long-lived launch instead inherits IO and on Unix replaces the process.
- Existing local/reclaim/managed tests cover tampering, races between acquisition callers, native leases, wrapper death, and Windows console interruption. Mock recipes demonstrate real release self-management. Main `openspec/specs` contains no published capability; the completed `reimplement-saucepan` deltas remain the baseline.

## Goals / Non-Goals

**Goals:** Separate address lookup, integrity checks, and resource lifetime while sharing one reclaim coordination mechanism. Keep fast access independent of payload byte count, retain strict validation at integrity boundaries, and give general consumers an ergonomic guard or process wrapper.

**Non-Goals:** Constant-time index parsing, a new database/graph, scheduled scrubbing, persisted verification timestamps, trusting mtime/size as cryptographic proof, permanent retention pins, a standalone lease-token service, SDK ports, automatic reacquisition on lookup, or a sandbox against other processes running as the same OS user. Existing acquisition/recovery hashing outside the access path is not a performance rewrite in this change.

## Decisions

### 1. Verification mode is a per-call access option

Introduce `VerificationMode::{Metadata, Full}` and a serializable `ResolvedResource { entry_uuid, directory, verification }` in `models/store` (a subordinate module is sufficient). Metadata is the default for access. It checks the authoritative UUID/type/state, contained root, and directory existence; it does not enumerate the payload. For a shim target, also validate the recorded binary declaration and relative path components and require a real regular file.

Full mode compares the existing complete file manifest through the same bounded inspector used today. A successful result explicitly reports `full`; failure returns a useful typed/error-chain diagnostic and never a successful verification result. No new persistent verification cache or policy field is introduced. In particular, these options never enter `Request`, item keys, fingerprints, source refresh state, or retention history.

Metadata success only confirms the address and basic availability at that moment. It does not promise every child exists or matches the acquisition. Full verification is a point-in-time check against local metadata, not authentication or protection against later external edits.

Alternative: retain full hashing on every lookup. Rejected because it makes normal access scale with all content bytes. Alternative: cached hashes inferred from mtime/size. Rejected because it adds invalidation rules and can mistake modified bytes for verified content.

### 2. Separate resolution, verification, and lifetime ownership

Proposed public API surface (names can be refined without changing these contracts):

```rust
Store::resolve(entry, mode) -> Result<ResolvedResource>
Store::path(entry) -> Result<PathBuf>                 // metadata convenience
Store::verify(entry) -> Result<ResolvedResource>      // full, temporary protection
Store::lease(entry, mode) -> Result<ResourceLease>
Store::lease_claim(app, claim, mode) -> Result<ResourceLease>
ResourceLease::resource(&self) -> &ResolvedResource
ResourceLease::directory(&self) -> &Path
```

`ResourceLease` lives in `core/store/access.rs` and owns native handles; it is neither serializable nor implicitly cloneable. Copying its path does not copy protection. A caller keeps the guard until all intended use finishes. Multiple independently acquired guards coexist. `resolve(..., Full)` acquires a temporary lease while checking, then drops it before returning the bare result; the returned path still has no continuing lifetime guarantee.

Retain the existing public `execution_lease(entry)` entry point as an adapter retaining its previous full-verification contract, so explicit existing callers do not silently lose their check. Internal launch uses the new metadata lease directly. Existing `path(entry)` keeps its Rust signature but intentionally changes its verification default; document that behavioral break and the full-mode replacement.

Alternative: expose a path and require callers to acquire a lock separately. Rejected because deletion can occur between those operations. Alternative: serialize guard tokens with an expiry. Rejected because neither elapsed time nor a token proves the consumer is alive.

### 3. Short mutation coordination, then entry-protected access

Lock order remains mutation lock then per-entry shared/exclusive lock. Under the mutation lock, complete pending recovery, read the exact output (or read app+claim/binding and its selected output), validate metadata, acquire the entry's shared coordination lock, and create the lifetime marker. Release the mutation lock before optional full scanning. Return the result only after the selected verification succeeds; unwind all guard resources on failure.

Reclaim keeps mutation-then-entry ordering, uses a nonblocking exclusive attempt, and checks native lifetime markers before marking content pending deletion. It never waits on a leased entry while holding the global lock. General resource use and execution use the same eligibility check and deferred report, preserving existing marker names for already-running installed shims.

A claim update/release after selection does not redirect an acquired lease. The consumer receives the selected UUID together with its corresponding directory. Unknown apps/claims and unavailable entries fail rather than silently using a different entry or making an acquisition.

This limits the new performance guarantee to ordinary access-time verification: no global mutation lock is held while its payload is scanned. Existing recovery may perform required integrity work before entry selection; it must remain correct even if that makes an exceptional first access slower.

### 4. Keep mandatory integrity paths explicit

Do not globally weaken or rename `verified_path` to a fast check and leave existing callers unchanged. Audit every caller in store acquisition, remote cached-copy reuse, publication recovery, activation, and launch. Introduce a distinct metadata resolver and a full checker with clear preconditions about the held lease/lock.

Publication/reuse/recovery continue to enforce their current manifest guarantees. Activation obtains one fully checked candidate lease before its probe, retains it across the probe, performs the necessary full post-probe check outside the global lock, then briefly reacquires mutation coordination to confirm ready state and publish the binding. Cooperating Saucepan operations cannot modify immutable payloads while leased; same-user external mutation is outside that guarantee. Failed validation still preserves the old binding. A probe that edits recorded files is caught by the post-probe scan.

Routine shim launch obtains one metadata lease from its committed binding and validates the target file without full-tree scanning. Changing the default does not authorize refresh, automatic activation, or selection by timestamps. Self-management inherits the same behavior; a previously installed shim is an old binary until separately maintained.

Alternative: delete the post-probe check as a duplicate. Rejected because probes can modify candidate content. Only repeated checking within the same validation phase is redundant.

### 5. CLI workflow for ordinary consumers

```text
saucepan --root ROOT path ENTRY_UUID [--verification metadata|full]
saucepan --root ROOT verify ENTRY_UUID
saucepan --root ROOT run ENTRY_UUID [--verification metadata|full] -- PROGRAM ARG...
```

`path` and `verify` print `ResolvedResource` JSON, retaining the existing `directory` field and adding entry/mode fields. `verify` is always full. Verification failures produce a nonzero exit and diagnostic, not a successful JSON result. `run` produces no wrapper JSON on stdout: child streams and status belong to the child. Source `--force-refresh`/`--allow-stale` options have no access-time effect.

`run` sets `SAUCEPAN_RESOURCE_DIR` only in the child environment, overriding any inherited value with the exact absolute directory. It forwards arguments literally and preserves the caller's working directory. Programs read the environment variable themselves; Saucepan does not interpolate `$...`, insert a shell, or substitute argument placeholders. Explicit root controls the store; arbitrary child arguments are untouched, unlike the dedicated Saucepan shim's existing root forwarding.

The CLI is a dedicated process boundary. Reuse the existing Unix exec and Windows spawn/wait semantics in a shared launcher helper under `core/launch.rs`; do not expose a seemingly ordinary library run method that unexpectedly replaces a host application's process. Native library consumers hold `ResourceLease` for in-process work. Native guards remain non-inheritable until the private dedicated-launch boundary opts into inheritance.

Preserve the tested target-retains-handle contract: Windows consumer inherits the sharing-protected marker, and Unix exec retains its lock descriptor. Failure to spawn/exec drops the lease. Wrapper death does not release the target's inherited ownership. Deliberate handle closure/daemonization outside this lifetime contract is unsupported and documented. Descendants inheriting the marker can conservatively extend protection. Do not offer unrestricted inheritance toggles to multithreaded native callers; unrelated spawns could accidentally extend lifetime.

### 6. Reuse utilities and verify observable work

No new utility family or dependency is justified. `utils/path::{beneath, plain_metadata}` provides containment/type checks; `utils/tree::inspect` supplies bounded full manifest reads; `utils/lock::LockGuard` supplies cooperating shared/exclusive coordination; `utils/process` remains for bounded probes. Move domain resource-lifetime composition out of launch into core access ownership, leaving OS mechanics behind a small private platform section as today. Do not place UUIDs, policies, claims, or reclaim decisions in utils.

Add focused tests around the existing filesystem/verification call boundary with a per-test observer or injectable reader sufficient to count payload opens/bytes and pause a full scan. Keep this seam local and avoid global mutable counters or a general filesystem abstraction. Assert zero payload traversal/reads for metadata resolution and one complete scan per requested full access. Also use real filesystem tampering tests: same-length modifications, missing/extra entries, unsafe components, and full-check read failures. Parsing large JSON manifests still costs work, so the claim is reduced payload IO, not constant-time resolution.

Use deterministic barriers rather than timing-only races for claim selection, lease-vs-reclaim, and unrelated access during a paused full scan. Extend actual child fixtures to consume data via `SAUCEPAN_RESOURCE_DIR`, retain the resource after a claim release, and demonstrate wrapper-death protection. Keep Windows hidden-console execution and Unix exec/signal cases separate and report unavailable OS runs honestly. Performance acceptance is based on observed file/byte work; elapsed-time samples are supplementary and have no fragile CI threshold.

## Risks / Trade-offs

- Metadata lookup misses payload tampering → explicitly label its verification level, provide full mode, and retain acquisition/activation checks. Never silently claim integrity.
- A caller drops a guard while keeping its path → document ownership with examples and use a guard-returning API rather than a separate lock call.
- Full validation races with external edits → retain the existing stable-source/trusted-local-user boundary; leases prevent Saucepan deletion, not arbitrary writes.
- Incorrect lock order deadlocks → centralize locked selection and nonblocking reclaim attempts; test with controlled interleavings.
- Leaked inherited handles delay reclaim → make ordinary guards non-inheritable and expose inheritance only in dedicated launch processes; native cleanup uses handle evidence rather than guessed timeouts.
- Broad replacement of `verified_path` weakens integrity → keep distinct internal helpers, audit all callers, and test corrupt reuse and probe mutation explicitly.
- Existing binaries retain old semantics → verify the new built shim in a fresh isolated test installation; report its digest. Do not silently replace an occupied mock or production shim as part of routine self-update.

## Migration Plan

Implement dependency-ordered checkpoints in tasks.md. Read existing new-format records without migration; do not add access choices to persisted requests. Update native and CLI documentation with `path`'s changed default, `--verification full`/`verify`, and ownership examples for `ResourceLease` and `run`.

Use temporary test roots for race and process tests. Extend acceptance recipes outside mock and run the new access commands against explicit isolated roots; existing mock contents must not be recursively reset. Build a fresh temporary managed installation to test the rebuilt shim, because replacing the existing stable shim is separate maintenance. Record Windows results and any Linux/macOS execution limitations, plus exact build identity and payload-read evidence. No implementation or mock mutation occurs during this proposal.

The completed baseline change remains unarchived. This proposal depends on its implementation but does not need to alter its artifacts; when archival is requested, publish/archive the baseline before this additive change. Rolling back binaries keeps persisted records readable, restores the previous full-check default, and removes availability of the new API/CLI surface without rewriting stored content.
