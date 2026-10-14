# Native scoped consumer

For one consumer using several prepared resources, use a [group lease](group-leases.md).
`AppSession::lease_group` checks every named claim/output under one app context,
optionally narrows access with referenced policies, and retains all selected outputs
under one guard. The CLI equivalent is `saucepan --as-app APP_UUID run-group group.json
-- PROGRAM`. Output-only trusted local groups also work without an application.

For the flat-policy native flow, use `AppSession::ensure` with
`models::claim::EnsureRequest`: supply an explicit `scope`, source UUID,
selector, content selection, optional context and registered `policy_refs`.
Use `selection.filter_refs` for ordered release asset filters. Obtain a guarded
covered-view lease from the returned claim/view using the existing session API;
the claim response's path alone does not extend the files' lifetime.

The standalone example and `tests/policy_acceptance.rs` use that same canonical contract. The acceptance fixture combines two scopes, explicit policy adoption, Git subpath updates, guarded copies, reclaim, exact recovery and a root-free no-store fetch.

Build the Rust example with `cargo build --offline --example scoped-consumer`.
Register the application/source using [the CLI examples](claim-cli.md), then
save an `EnsureRequest` with an explicit project scope and, optionally, a
`selection.folder` for the package directory. Run:

```powershell
cargo run --offline --example scoped-consumer -- ./scratch APP_UUID ensure.json resource.txt
```

An optional final argument supplies a binary credential file when proof is enabled.
The example obtains the app session, ensures ownership, acquires a fully verified
covered-view lease, and reads a relative file while the guard remains alive.
It reports claim/output identities and byte count; errors expose stable codes.
Rust scope exit and error propagation drop the native guard automatically.

Use `project-a` and `project-b` in two request files for independent ownership.
Compatible requests in one scope share a claim, including narrower natural
directory views. The two scopes may initially share output bytes; updating one
does not update the other. Find the intended claim through an app-scoped context
query and lease it before reading; never select from global output history.
When `content_unavailable` is returned, choose a registered recovery strategy and
call explicit recovery (or ensure) rather than assuming the last observed path
still exists. Queries and ordinary leases do not trigger downloads.

Run the executable fixture:

```powershell
cargo test --offline --test scoped_consumer
cargo test --offline --test claim_cli --test run
```

The fixture starts actual Rust consumer processes that continuously read a nested
package while another Saucepan CLI process reclaims outputs. It verifies separate
scope updates, one claim for a covered request, whole-output protection, query
selection, claim release, normal return, error return and abrupt process death.
Failed full verification and unavailable reads leave no orphan ownership.

For a CLI-launched consumer, `run-claim APP_UUID CLAIM_UUID --relative packages/tool
-- COMMAND` sets `SAUCEPAN_RESOURCE_DIR` and passes the existing native lifetime
handle to the child. Children must retain inherited handles; intentionally closing
them or escaping the process lifetime is unsupported. Native Rust guards are not
inherited by unrelated child processes. Windows x86_64 execution is verified;
Linux/macOS runtime coverage is not claimed. No language binding or changes to
the Powerspec repository are needed for this fixture.
