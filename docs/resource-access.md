# Resource access

Native consumers keep a `ResourceLease` alive for the entire period of use:

```rust,no_run
use saucepan::core::Store;
# fn example(store: &Store, app: uuid::Uuid, claim: uuid::Uuid) -> anyhow::Result<()> {
let resource = store.lease_claim(app, claim)?;
let bytes = std::fs::read(resource.directory().join("data"))?;
// Reclaim defers this exact output until the final guard is dropped.
drop(resource);
# Ok(())
# }
```

`lease(entry_uuid)` selects an exact output directly. Claim selection and lease
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
