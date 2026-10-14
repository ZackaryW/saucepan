# Group leases

A group retains already prepared outputs for one consumer. It does not retrieve,
update, recover, register an application or create claims. Prepare resources first.

An output-only request needs no application registration or credentials:

```json
{
  "members": [
    {"name": "command", "target": {"type": "output", "uuid": "11111111-1111-4111-8111-111111111111"}},
    {"name": "hooks", "target": {"type": "output", "uuid": "22222222-2222-4222-8222-222222222222"}, "relative": "hooks"}
  ]
}
```

Use real output UUIDs from acquisition receipts. A claim target uses
`{"type":"claim","uuid":"33333333-3333-4333-8333-333333333333"}` and requires
an explicit app session. Every member is checked under the same session, including
direct output targets. An unavailable or denied member fails the whole admission.

There must be 1–128 members with unique case-sensitive names matching
`[A-Za-z][A-Za-z0-9_-]{0,63}`. Names label results; they do not grant scope access.
Optional `relative` values identify directories within an output's manifest.
Two names may refer to the same output with different directory views.
Unknown fields and invalid paths are rejected. CLI input is limited to 1 MiB.

## Optional boundaries

Add `policy_refs` containing registered or caller-catalog policy UUIDs. Only access
policies and one path verification policy are accepted. For example, a portable
catalog can contain:

```json
[
  {
    "uuid": "44444444-4444-4444-8444-444444444444",
    "type": "access_policy",
    "actions": ["lease"],
    "scopes": ["project-a"]
  },
  {
    "uuid": "55555555-5555-4555-8555-555555555555",
    "type": "verification_policy",
    "policy_target": "path",
    "mode": "full"
  }
]
```

Reference these UUIDs in the request. Scope restrictions require claim targets;
direct output targets have no scope witness. Multiple access policies intersect.
An omitted dimension is unrestricted; an empty list denies all values of that
dimension. The boundary also applies to trusted local requests. It never expands
registered permissions or supplies identity. Caller catalogs cannot shadow
registered UUIDs, and changing a policy requires a new UUID and explicit adoption.

Group verification is a minimum: `metadata` cannot weaken a member's existing
`full` requirement. Shared outputs are checked once at the strongest required mode.
Descriptors report the actual mode and policy provenance. These are informational
paths, not authorization tokens or serialized lifetime guards.

This is a cooperative lifetime and policy boundary, not a filesystem sandbox.
Persistent groups, automatic retrieval and atomic multi-resource updates are
separate future changes.

## Native consumers

`Store::lease_group(&request, &catalog)` supports trusted local output access.
`store.session(app_uuid, credential)?.lease_group(&request, &catalog)` applies the
app's current restrictions to every member. Pass `PolicyCatalog::default()` when
all references are registered or omitted. Credentials are required only for apps
that opted in.

Keep the returned `GroupLease` alive while using `group.member("hooks")?.directory`.
Its descriptor maps names to paths, selected output/claim UUIDs, verification and
policy provenance. Duplicate-output names share one guard but keep independent
directory views. Dropping the guard releases all outputs together. Native guards
are not inherited by unrelated child processes.

Admission selects all resources under coordination, scans outside global
coordination, then rechecks relevant app and source revisions. Concurrent updates,
release, uninstall, credential changes or policy adoption fail admission cleanly.
Unrelated policy registration does not invalidate it. Once admitted, a group pins
its original outputs even when claims subsequently change or disappear; a later
group sees the new selection. Reclaim waits for every retaining consumer to finish.

Run the native example against prepared outputs:

```text
cargo run --example group_consumer -- ROOT GROUP_JSON
cargo run --example group_consumer -- ROOT GROUP_JSON APP_UUID
```

The example in `examples/group_consumer.rs` opens each live directory and drops the
guard explicitly. A copied descriptor does not retain files after that drop.

## CLI consumers

Build `cargo build --example group_reader`, then launch the example against a
prepared request. Windows uses the `.exe` suffix; Unix omits it:

```text
saucepan run-group group.json -- target/debug/examples/group_reader.exe
saucepan --as-app APP_UUID run-group group.json --policies policies.json -- target/debug/examples/group_reader.exe
```

The default store is the user's `~/.saucepan`. Use `--root PATH` for a disposable
or alternate store. For a protected app, add `--credential-file PATH`. Requests
and portable catalogs each have a 1 MiB limit; the resulting descriptor is bounded
to 16 MiB. Rejected admission writes no successful paths and starts no child.
`--error-format json` exposes the existing structured errors.

The child gets `SAUCEPAN_RESOURCE_GROUP_FILE`, an absolute path to an owned UTF-8
JSON file with a `members` object. Each name maps to `output_uuid`, optional
`claim_uuid`, `directory`, `relative`, `verification` and `policy_provenance`.
No credential or native handle value is included. Saucepan removes stale singular
`SAUCEPAN_RESOURCE_DIR` and `SAUCEPAN_SOURCE_PATH/KIND/COMMIT` variables from the
child environment. Parent environment, working directory, literal arguments and
stdin/stdout/stderr are preserved. The child's exit status is propagated.

Each distinct output and the descriptor directory have inherited native ownership.
On Windows, killing the waiting wrapper does not release a live child's resources.
On Unix the dedicated wrapper executes the child in place. Closed output markers
are swept by reclaim; closed descriptor scratch is swept on later consumer launch.
Setup/spawn failure releases ownership, making any remaining markers reclaimable.
Children must retain inherited handles for the duration of use; closing handles
or detaching work that discards them ends that protection. This behavior is confined
to the dedicated CLI boundary; native library calls do not make handles inheritable.

`tests/group_leases.rs` exercises three separate command/hook/service outputs,
optional app identity, a scoped boundary, duplicate directory views, claim updates,
release, reclaim and wrapper termination. It is a disposable Zush-shaped fixture;
it does not modify Zush or Power Eagle. Windows execution is verified here;
Linux/macOS execution requires their corresponding test runs.
