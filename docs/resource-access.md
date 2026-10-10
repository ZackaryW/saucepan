# Resource access

CLI commands default to the user-home `.saucepan` store; `--root ROOT` is an
optional override. App consumers use `Store::session(app, optional_credential)`
and keep the returned `AppSession` for queries, claims and resource access.
Credentials are opt-in; registered permissions still apply without them.

`saucepan --root ROOT deactivate APP_UUID` clears only the application's active
binding. It returns `{ "app": "UUID", "deactivated": true, "previous_binding":
{ "entry_uuid": "UUID", "executable": "tool.exe" } }`. Repeating it succeeds
with `deactivated: false` and `previous_binding: null`, without rewriting the
application. The app-facing native API is `session.deactivate()`, returning
`Result<Option<Binding>>`. The lower-level `Store::deactivate(app)` also exists.
Missing application UUIDs and UUIDs belonging to outputs fail.

Claims, settings, installed shims, and stored content remain unchanged. Clearing
a binding does not require the old payload to be healthy. A launch that already
selected and leased the binding can finish normally; later shim launches fail
with `application has no active binding`. A subsequent `reclaim` still respects
retention, other applications' active bindings, and live leases. Deactivation
does not itself reclaim content.

Normal activation can restore a binding and reuse the installed shim. Mutations
serialize at commit: an activation that commits after deactivation, including
one whose probe was already running, can reactivate the app. The CLI's boolean
reports whether that invocation removed a binding, not a permanent disabled
state. If deactivating Saucepan itself, use a directly invoked retained binary
to reactivate it; its inactive shim cannot launch that command.

Routine managed launch selects the committed active binding, obtains one
metadata lease, and checks that its declared executable is a regular contained
file. It does not hash the payload. Activation and rollback still scan the full
candidate before probing and again after probing; a probe that modifies content
cannot publish a new binding. Both scans retain the entry lease without holding
the global mutation lock. Publication, recovery, and compatible-output reuse
keep their full integrity checks. Existing installed shims retain their old
behavior in their separate historical roots. Rebuild and install shims for the
current store integrity contract; earlier roots are not migrated.

`path ENTRY_UUID` now defaults to `metadata`: it checks identity, ready state,
and a real contained output directory, without walking or hashing its contents.
Use `path ENTRY_UUID --verification
full` or `verify ENTRY_UUID` when you need a complete manifest comparison.
Both commands return JSON containing `entry_uuid`, `directory`, and
`verification`. Unknown modes and failed checks return an error with no success
JSON. These commands always address the supplied UUID, even after newer pulls.

Trusted store-level equivalents are `resolve(entry, VerificationMode::Metadata)`,
`resolve(entry, VerificationMode::Full)`, and `verify(entry)`;
`path(entry)` remains a metadata convenience returning just a `PathBuf`.
`lease` and `lease_claim` accept the same verification mode, and
`guard.resource()` reports the selected UUID, directory, and successful mode.
The existing `execution_lease(entry)` adapter retains full verification.

Full access holds a temporary or retained lease while scanning once, without
holding the global mutation lock. It detects missing, extra, and modified content
against the recorded manifest. Payload comparison complements the store-selected checksum or HMAC verification
of metadata; it is a point-in-time check and cannot prevent subsequent
external edits. Verification does not refresh sources or change stored requests,
identities, creation order, retention, or claims. Bare resolution drops its
temporary protection before returning, even in full mode.

Native app consumers keep a `ResourceLease` alive for the entire period of use.
This example assumes an application whose credentials are disabled and whose
policies permit leasing the claim; provide its credential when proof is enabled:

```rust,no_run
use saucepan::{core::Store, models::store::VerificationMode};
# fn example(store: &Store, app: uuid::Uuid, claim: uuid::Uuid) -> anyhow::Result<()> {
let session = store.session(app, None)?;
let resource = session.lease_claim(claim, VerificationMode::Metadata)?;
let bytes = std::fs::read(resource.directory().join("data"))?;
// Reclaim defers this exact output until the final guard is dropped.
drop(resource);
# Ok(())
# }
```

`session.lease_output(entry_uuid, VerificationMode::Metadata)` selects an exact
output accessible through that app's authorized claims. `Store::lease` without
a session is trusted local direct-output access. Claim selection and lease
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

Use `--as-app APP_UUID` with `path`, `verify` or `run` for restricted direct-output
access. Without an app, these exact-output commands use trusted local
administration. `run-claim APP_UUID CLAIM_UUID [--relative FOLDER] -- PROGRAM`
selects a claim or covered view through its app session; the app's optional
credential comes from `--credential-file FILE`. See [scoped consumers](scoped-consumer.md).

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
