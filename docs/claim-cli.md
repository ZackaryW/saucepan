# Application claim CLI

Use a fresh root. Initialization defaults to basic checksum integrity without an
OS keyring. Select `init --integrity keyring` for native-backed HMAC protection.
Application credentials are independently optional within keyring mode; basic
stores do not support credential-protected registrations. Earlier store
layouts are not migrated.
The examples select `./scratch` explicitly; omit `--root` to use the user-home
`.saucepan` store.

Create `policies.json` with the shared definitions:

```json
[
 {"uuid":"10000000-0000-4000-8000-000000000001","type":"access_policy","scopes":["project-a","project-b"]},
 {"uuid":"10000000-0000-4000-8000-000000000002","type":"claim_visibility_policy","public_scopes":["project-a"]},
 {"uuid":"10000000-0000-4000-8000-000000000003","type":"policy_constraints","maximum_keep":20},
 {"uuid":"10000000-0000-4000-8000-000000000004","type":"reclaim_policy","strategy":"restore_exact"}
]
```

Create `registration.json`:

```json
{"registration_key":"my-consumer","name":"Consumer","credentials_required":false,
 "policy_refs":["10000000-0000-4000-8000-000000000001","10000000-0000-4000-8000-000000000002","10000000-0000-4000-8000-000000000003"]}
```

```powershell
saucepan --root ./scratch init
saucepan --root ./scratch register-policies policies.json
saucepan --root ./scratch register-application registration.json
```

The result is application metadata, including its UUID. Repeating the same
registration key and settings returns the same app. Display names are not identity.
`register-app NAME` is a trusted convenience that allocates a new registration key;
integrations should use the explicit-key command above.

Register a source, then place its UUID in `ensure.json`:

```json
{"scope":"project-a","source_uuid":"SOURCE_UUID","selector":{"type":"local"},
 "selection":{},"context":{"package":"example"},
 "policy_refs":["10000000-0000-4000-8000-000000000004"]}
```

```powershell
saucepan --root ./scratch register-source source.json
saucepan --root ./scratch ensure-claim APP_UUID ensure.json
saucepan --root ./scratch recover-claim APP_UUID CLAIM_UUID
saucepan --root ./scratch run-claim APP_UUID CLAIM_UUID -- consumer.exe
saucepan --root ./scratch --as-app APP_UUID path OUTPUT_UUID
```

Ensure returns `claim`, `view`, and `acquired`. A contained directory can return
the same owner plus `view.relative`. `run-claim` retains the whole output while the
child uses `SAUCEPAN_RESOURCE_DIR`. Ordinary paths and JSON results do not retain
a lease. `acquire --app APP_UUID --scope SCOPE` remains supported for complete
acquisition requests with explicit policy values; it does not introduce an older
persisted schema. Prefer `ensure-claim` for registered defaults.
For a directory claim, add `--relative packages/example` only when that folder
exists in the owning output; a whole-output or single-file claim needs no relative
view.

`query-claims FILE` accepts `{"app":"APP_UUID","scope":"project-a","where":[]}`;
omitting scope searches permitted scopes within that app. `discover-claims FILE`
accepts `{"where":[]}` and exposes only explicitly public credential-free scopes.
Discovery does not grant ownership or direct-output access.

## Optional credentials and administration

On a root initialized with `--integrity keyring`, use a new registration key, set
`credentials_required` to true and choose a new secret destination. To enable
credentials on the app registered above, use `configure-application` as described
below; retrying registration with changed settings is a conflict.

```powershell
saucepan --root ./scratch register-application registration.json --credential-out ./consumer.credential
saucepan --root ./scratch --credential-file ./consumer.credential ensure-claim APP_UUID ensure.json
```

Credential files contain exactly 32 binary bytes. Tokens are never printed or
accepted as literal command arguments. Output uses create-new semantics and is
restricted to its owner (Unix mode 0600; Windows protected owner-rights DACL)
before writing the secret. Delivery is flushed before registration/rotation is
committed. Failed delivery leaves metadata unchanged. An empty or partial file
may remain after an I/O failure; choose a new destination on retry. A crash after
delivery may leave an unused token file; retry registration with that file to
discover whether the registration committed. Existing credentials are never
redisclosed. Filesystem protection remains the OS's responsibility.

`configure-application APP_UUID FILE --credential-out NEW_FILE` is trusted local
administration. The JSON contains `expected_revision`, the complete `policy_refs`
list, and optional `credential`: `enable`, `rotate`, or `disable`.
Supply current proof with `--credential-file` when already enabled. A stale
revision or a policy that invalidates a surviving claim fails without mutation.
`--as-app` cannot be used to turn an app operation into configuration or global
maintenance. Commands without an app that inspect/maintain global store state are
trusted administration; integrations use app-scoped commands or `--as-app` for
direct output access. Discovery is the exception: `query-sources` without
`--as-app` and `discover-claims` expose only their explicitly public candidates,
not an administrative list. This is not isolation from another process of the same OS user.

## Machine errors

`--error-format json` emits one stderr object on a command/preflight failure:

```json
{"error":{"code":"content_unavailable","message":"selected claim content is unavailable","details":{}}}
```

There is no success stdout on failure. Validly supplied JSON error mode also
covers malformed arguments and root-resolution failures. Omitting `--root` selects
the user-home `.saucepan` store; a missing store reports initialization guidance.
Help and version exit 0.
Lifecycle results retain stdout receipts and exit 0 for complete or 2 for
incomplete/failed/conflicted work; `reason` contains a structured code when work
fails or needs reconciliation. A launched child's streams and exit code are
forwarded unchanged, including under JSON error mode. See the code catalogue in
[claim-policies.md](claim-policies.md) and executable subprocess examples in
`tests/claim_cli.rs` and `tests/cli.rs`.
