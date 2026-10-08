# Resource access

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
