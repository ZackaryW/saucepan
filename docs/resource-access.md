# Resource access

Routine managed launch selects the committed active binding, obtains one
metadata lease, and checks that its declared executable is a regular contained
file. It does not hash the payload. Activation and rollback still scan the full
candidate before probing and again after probing; a probe that modifies content
cannot publish a new binding. Both scans retain the entry lease without holding
the global mutation lock. Publication, recovery, and compatible-output reuse
keep their full integrity checks. Existing installed shims retain their old
behavior until separately rebuilt and installed.

`path ENTRY_UUID` now defaults to `metadata`: it checks identity, ready state,
and a real contained output directory, without walking or hashing its contents.
This changes the earlier full-check default. Use `path ENTRY_UUID --verification
full` or `verify ENTRY_UUID` when you need a complete manifest comparison.
Both commands return JSON containing `entry_uuid`, `directory`, and
`verification`. Unknown modes and failed checks return an error with no success
JSON. These commands always address the supplied UUID, even after newer pulls.

Native equivalents are `resolve(entry, VerificationMode::Metadata)`,
`resolve(entry, VerificationMode::Full)`, and `verify(entry)`;
`path(entry)` remains a metadata convenience returning just a `PathBuf`.
`lease` and `lease_claim` accept the same verification mode, and
`guard.resource()` reports the selected UUID, directory, and successful mode.
The existing `execution_lease(entry)` adapter retains full verification.

Full access holds a temporary or retained lease while scanning once, without
holding the global mutation lock. It detects missing, extra, and modified content
against the recorded manifest. It is a point-in-time integrity check against
local metadata, not authentication or protection against subsequent external
edits. Verification does not refresh sources or change stored requests,
identities, creation order, retention, or claims. Bare resolution drops its
temporary protection before returning, even in full mode.

Native consumers keep a `ResourceLease` alive for the entire period of use:

```rust,no_run
use saucepan::{core::Store, models::store::VerificationMode};
# fn example(store: &Store, app: uuid::Uuid, claim: uuid::Uuid) -> anyhow::Result<()> {
let resource = store.lease_claim(app, claim, VerificationMode::Metadata)?;
let bytes = std::fs::read(resource.directory().join("data"))?;
// Reclaim defers this exact output until the final guard is dropped.
drop(resource);
# Ok(())
# }
```

`lease(entry_uuid, VerificationMode::Metadata)` selects an exact output directly. Claim selection and lease
acquisition share mutation coordination with reclaim. Updating or releasing a
claim does not redirect an existing guard. Multiple guards can coexist; the last
guard's release restores normal retention eligibility. Leases do not change
creation order, provenance, or claim policy. The `tests/access.rs` example reads
a leased file after its claim is released and retention is exceeded, then drops
the final guard and observes reclaim.

A copied path carries no protection. Neither an ordinary claim nor a bare
`path` result pins content. Guards protect against Saucepan reclaim, not local
external edits. Native guards are non-inheritable; managed process launch opts
into handle inheritance only at its dedicated launch boundary.

Metadata lookups create no lifetime markers. Native guards and full scans use
shared entry coordination without creating per-use markers. Only a dedicated
process launch creates an inherited lifetime marker, while the shared guard
continues protecting the output throughout the handoff to the child.

Reclaim (including acquisition's automatic reclaim pass) also sweeps closed
process markers for retained or active outputs. Busy entry locks and live native
handles defer cleanup. This handles older launchers' markers and process crashes
without relying on PIDs or timeouts. No background sweep runs when Saucepan is
idle. Stable coordination lock files are retained and reused.

For external tools, use `run ENTRY_UUID [--verification metadata|full] -- PROGRAM
ARG...`. Saucepan sets `SAUCEPAN_RESOURCE_DIR` in the child's environment,
overriding an inherited value without changing the parent environment. Arguments
remain literal, the working directory is inherited, and child streams and exit
status pass through without wrapper JSON. No shell or placeholder expansion is
performed. `--force-refresh` and `--allow-stale` have no effect on access.

The included example reads a `data` file from that environment directory:

```powershell
cargo build --example read-resource
target/debug/saucepan --root ROOT run OUTPUT_UUID -- target/debug/examples/read-resource.exe
```

On Unix omit `.exe`. The native example above and this process workflow both
remain protected after claim release and outside the retention window.
The example's optional `CONSUMER_READY` and `CONSUMER_RELEASE` file handshake
lets acceptance hold it open while exercising reclaim. Ordinary exit makes its
output eligible for reclaim again.

`run` and shims share native inherited ownership: Windows spawns/waits, while
Unix replaces the wrapper via `exec`. Killing the Windows wrapper alone does not
release a handle still retained by the child. Targets that deliberately close
inherited handles or daemonize outside this contract are unsupported. Descendants
that retain handles can extend protection. Ordinary native guards do not pass
ownership to unrelated child processes; use the dedicated CLI boundary when
that lifetime is needed. Failed validation or process creation releases ownership.
