# Saucepan Python SDK

A synchronous, standard-library-only Python 3.9+ client for the Saucepan 0.5.x central-store CLI. It uses an independently installed executable, defaulting to `~/.saucepan/bin/saucepan` (`saucepan.exe` on Windows). It never downloads or builds the executable at runtime.

## Install and acquire

Install from this checkout, or from a pinned copy of `sdk/python/`:

```sh
pip install ./sdk/python
```

```python
from pathlib import Path
from saucepan_sdk import Saucepan

store = Saucepan()  # Or Saucepan(binary=Path("/path/to/saucepan"))
store.init()       # First initialization only; fails if the store already exists.
token = store.register("example-app")  # Register once, then save/reuse the token.
app = store.for_app(token)
result = app.acquire({
    "source": {"provider": "git", "origin": "https://github.com/github/gitignore.git", "reference": "main"},
    "folder": "Global",
})
print(Path(result["directory"]))
app.verify(app.view())
```

The CLI owns native-keyring access, encrypted app settings, source reuse, filtering, and snapshot retention. Recipes and responses are ordinary JSON-compatible Python dictionaries. The SDK does not cache settings or views. See the [central-store guide](../../docs/central-source-store.md) for source formats and policy behavior.

## Caller context

`store.for_app("example-app")` selects an ordinary app context. `store.for_app(token)` selects the authenticated context returned by registration. A new client can use `Saucepan(token=token)` or `Saucepan(marker=Path(".saucepanhash"))`; marker and token are alternatives. The caller owns any persisted token file, and the SDK never rewrites it. A token stays valid as the app's settings and entries change.

`authoritative=True` requires a valid token/marker at the CLI boundary. Filters select touched entries in the app view; they are not acquisition permissions. `for_app()` keeps the executable, timeout, and test-store options while replacing the caller context.

```python
app.configure(
    settings={"retain_snapshots": True, "verify_content": True, "allow_local_fallback": False},
    filters={"source_ids": [], "providers": ["git", "url", "local"]},
)
```

Both `register()` and `configure()` accept optional `settings=` and `filters=` dictionaries. When supplying settings, include all three fields. The CLI persists them centrally; no workspace TOML is read.

## Methods and results

| Method | Parsed CLI result |
| --- | --- |
| `init()` | `{"created": True}` |
| `register(app, settings=..., filters=...)` | Stable app token |
| `configure(settings=..., filters=...)` | `{"configured": True}` |
| `acquire(recipe)` | Artifact, central directory, and verification/fallback status |
| `view()` | Current app settings, filters, and touched entries |
| `verify(view)` | `{"verified": True}`; raises on stale or altered views |
| `path(artifact_id)` | Central directory string or `None` |
| `mirror(artifact_id, destination)` | `{"directory": ...}`; destination may be a `Path` |
| `history(source_id)` | Current/history metadata or `None` |
| `snapshot(source_id, snapshot_id, folder=None)` | Exact retained acquisition, without advancing current |
| `shared_executable()` | Shared executable path reported by the CLI |

`shared_executable_path()` is also exported for computing the default executable location without invoking the CLI. Returned content paths always come from the CLI.

Git folder selections share source snapshots. Safe committed Git symlinks resolve to ordinary files/directories against the complete source before selection. Local filesystem links and downloaded ZIP symlink entries remain rejected.

## Errors and request handling

`SaucepanError` carries `exit_code`, `stdout`, `stderr`, and an optional `code`. Core failures currently exit 1 and CLI usage errors exit 2. Missing paths returned as JSON `null` become `None`. Process launch failures and timeouts have no exit code; invalid JSON and unsupported token/view format versions fail explicitly.

```python
from saucepan_sdk import SaucepanError

try:
    app.acquire({"source": {"provider": "local", "path": "./assets"}})
except SaucepanError as error:
    print(error.exit_code, error.stderr)
```

Set `timeout=30` for a 30-second per-command timeout; the default `None` waits without a deadline. Calls use argument arrays without a shell. Concurrent calls own separate temporary JSON files, removed after success or failure. Supplied tokens are copied into the client context; request files use private creation permissions. Process-error messages do not reproduce command arguments containing test keys.

## Isolated testing

```python
store = Saucepan(binary="/path/to/saucepan", test_root="/tmp/my-test-store", test_key="12" * 32)
store.init()
```

Both test options are required together; the key must contain exactly 64 hexadecimal characters. Use disposable test keys and a short root on Windows. Explicit test mode never becomes an automatic production fallback.

```sh
uv run --project sdk/python pytest sdk/python/tests
uv run --project sdk/python behave sdk/python/features/python-sdk
uv build --project sdk/python
```

The tests build the actual CLI or use `SAUCEPAN_TEST_BINARY`. They run isolated stores without accessing the user's keyring. Vendoring checks import and invoke the SDK with site packages disabled. SDK CI runs on Windows, Linux, and macOS.

## Migration from the old Python SDK

Version 0.5.0 replaces the old `Workspace`, `Sauce`, and `Bucket` API. Existing callers must migrate; these names and the old five-category exception mapping are removed. Register an app, supply a recipe to `acquire()`, and consume the returned artifact/directory. Replace local TOML settings with `register()` or `configure()`, and index reads with `view()`. There are no bucket, install, update, or uninstall wrappers for commands the current CLI does not provide.

Only `sdk/python/` is needed to vendor this package. Supply a compatible CLI independently; SDK tests additionally need that executable or a full source checkout to build it.
