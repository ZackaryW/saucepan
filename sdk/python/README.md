# Saucepan Python SDK

Python 3.12+. Install this directory with `pip install ./sdk/python` from the repository root, or `pip install -e ./sdk/python` for development. A separately installed Saucepan executable is required only when an operation runs. Import and client construction do not locate it, create a store or contact a keyring.

```python
from saucepan_sdk import Client
client = Client()  # Saucepan on PATH; Rust selects the default root.
scoped = client.for_app("your-registered-app-uuid")  # no invented claim scope
```

`executable` and `root` accept explicit strings. Credentials are optional caller-owned file paths on `for_app`; credentials are never copied into SDK scratch. Python methods are synchronous. No legacy SDK aliases or native language bindings are provided.

The transport uses literal argv and per-call closed JSON files. JSON calls default to 120 seconds, 16 MiB per captured stream and 16 MiB per input (group/pin inputs are additionally capped at 1 MiB). Configurable limits must be positive. Integers must fit the shared JavaScript safe range. A transport failure exposes `kind` and `indeterminate`; a mutation may already have committed. Inspect the original record/key before explicitly retrying. No automatic retries or compensating releases occur.

`SaucepanError` exposes Rust's `code`, `message`, and `details`; exception rendering omits remote diagnostic text. `TransportError` distinguishes protocol, missing executable, launch, timeout and bounds failures. An injected `Transport` owns its process implementation; it must preserve these same contracts.

Guarded runs inherit streams and have no implicit timeout. Captured run streams are bytes, never parsed as JSON. Explicit cancellation kills/reaps the SDK-owned wrapper, not necessarily its descendants. Durable pins require explicit unpin; returned paths alone provide no retention.

Development: `uv run --group dev python -m unittest discover -s tests` and `uv run --group dev mypy` from this directory. Standard library process/files/typing cover runtime needs; Hatchling builds packages, mypy checks types and unittest exercises behavior. Pydantic, Execa-equivalents and async wrappers are deliberately absent because Rust owns semantic validation.

## Resource workflow

```python
client = Client(executable="/path/to/saucepan", root="/disposable/store")
client.init()
source = client.register_source({"type": "git", "origin": "/path/to/local/repo"})
app = client.register_application({"registration_key": "my-tool", "name": "My tool"})
bound = client.for_app(app["uuid"])
selected = bound.ensure_claim(app["uuid"], {
    "source_uuid": source["uuid"], "selector": {"type": "ref", "reference": "HEAD"},
    "scope": "project-a", "selection": {"folder": "skills"},
})
views = bound.query_claims({"app": app["uuid"], "scope": "project-a", "where": []})
# Lookup does not refresh. Updating is an explicit operation on this claim.
updated = bound.update_claim(app["uuid"], selected["claim"]["uuid"])
```

`query_sources()` requires neither an app nor a JSON file; anonymous discovery sees only explicitly public sources. To use unrestricted local acquisition, call `acquire(request)` on an unbound client. An app-bound `acquire` requires an explicit scope. Raw metadata results preserve Rust's wire names; inputs use the TypedDict declarations in `saucepan_sdk.models`.

`retrieve` returns `{request, descriptor}` describing original material; `source_path` looks up its address without fetching. `acquire_retrieved` returns copied/extracted results. Inline no-store operations accept an explicit portable `policies` catalog and caller-owned destinations without opening a root. Temporary content uses `run_retrieved` or `run_acquired` to retain its lifetime.

For persistent sets, call `register_group`, then `prepare_group`. A `pin` argument is explicit and requires `expected_revision` and a registered `pin_policy`. Inspect with `pin`/`pins`, then call `unpin` only when all readers of those exact paths are finished. Ordinary group preparation creates no retention promise. `run_group`, `run_registered_group` and `prepare_run_group` retain resources through Rust's native process boundary.

Trusted administration can supply `credential_file` to `Client` when configuring a credential-protected app; that proof does not automatically select an app identity. App-bound calls use `for_app(app, credential_file=...)`. Conflicting app arguments fail before execution. No credential bytes are loaded by this library.

## Profiles and managed applications

`client.profile()` expands the manager's `managed` recipe without opening a root; `profile('explicit-update', for_install=True)` prepares ordinary policies for an executable install. Call `register_policies(profile['catalog'])` explicitly and attach the returned refs. `explain_policies(request, policies=catalog)` returns effective choices and provenance without fetching files. Inputs/results are typed in `saucepan_sdk.models.setup`.

`install_application(name, request, probe, built_shim, scope=..., shim=...)`, `bootstrap(request, built_shim)`, `update_application`, `self_update`, `rollback`, `uninstall` and `reconcile` return `{'exit_code': 0 | 2, 'result': ...}`. Exit 2 preserves a recorded noncomplete outcome, including failed/conflict status. `operations(app=...)` returns a read-only list. Inspect receipts before explicitly reconciling or retrying the same `operation` UUID. No automatic downloads, PATH changes, retries, credentials or scopes are introduced. Standalone shim maintenance and activation remain CLI-only. See [the SDK guide](../README.md#profiles-and-managed-setup) and runnable `examples/profiles.py` / `examples/managed_setup.py`.

Real-binary tests use `SAUCEPAN_TEST_BINARY` (otherwise the repository's debug build) and its sibling shim, isolated temporary roots, local Git and a loopback release fixture. No external service or real consumer repositories are needed. Cross-platform and external consumer integration remain separately reported evidence.
