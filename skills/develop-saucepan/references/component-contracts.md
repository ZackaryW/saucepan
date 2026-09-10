# Saucepan component contracts

Use this reference when integrating Saucepan or changing its public behavior. Verify every detail against the checked-out revision when working with a commit newer than this skill.

## CLI contract

Invoke the CLI as:

```text
saucepan <workspace-root> [--json] <command>
```

The workspace root contains `saucepan.toml`. A minimal local-only configuration is:

```toml
[local]
```

Primary commands:

| Command | Result |
|---|---|
| `install <target> [--ref <ref>]` | Install a sauce and record requested/resolved refs |
| `update <name>` | Refresh a non-local sauce |
| `uninstall <name>` | Remove a sauce safely |
| `list [--json]` | List installed sauces; JSON mode is NDJSON |
| `path <name>` | Print the bare installed repository root |
| `search <jq-filter>` | Emit each raw parsed jq result |
| `bucket add\|remove\|list` | Manage bucket documents |
| `cat index\|buckets\|sauce\|bucket` | Emit raw JSON state |

Stable exit codes:

| Code | Meaning | SDK exception |
|---:|---|---|
| 0 | Success | — |
| 1 | Not found | `NotFound` |
| 2 | Source failure | `SourceError` |
| 3 | Invalid or missing configuration | `ConfigError` |
| 4 | Installation conflict | `Conflict` |
| 5 | Unexpected internal error | `InternalError` |

Preserve machine-readable output, NDJSON boundaries, stderr diagnostics, and exit meanings. Middleware callers rely on them.

## Artifact documents

Every installed repository exposes `sauce.json` with required fields and arbitrary preserved extension fields:

```json
{
  "name": "my-tool",
  "version": "1.2.0",
  "description": "A short description"
}
```

A `bucket.json` is an array of searchable stubs:

```json
[
  {"name": "my-tool", "version": "1.2.0", "url": "https://github.com/owner/my-tool"}
]
```

Treat a manifest's `name` as the installed identity; it may differ from the repository target.

## Binary acquisition boundary

Binary acquisition logic has moved outside this repository. Saucepan no longer
ships a Python binary resolver. Consumers supply a compatible executable through
their external acquisition mechanism or build the selected Rust revision.

## Python SDK contract

`sdk/python` requires Python 3.9 or newer, has no runtime dependencies, and never downloads or builds the CLI. Its public exports are:

```python
from saucepan_sdk import (
    Bucket,
    ConfigError,
    Conflict,
    InternalError,
    NotFound,
    Sauce,
    SaucepanError,
    SourceError,
    Workspace,
)
```

`Workspace(root, binary=None)` resolves `saucepan` from `PATH` unless `binary` is supplied. Command failures retain `exit_code` and full `stderr`.

Important behavior:

- `workspace.sauces` and `workspace.buckets` are independently lazy-cached snapshots.
- Every mutation invalidates both caches.
- `Sauce.update()` refreshes that same object in place.
- `workspace.refresh()` discards both caches and immediately reloads the sauce index.
- `search()` returns raw parsed JSON values without coercion.
- `Sauce.path` delegates to the CLI `path` command; never duplicate layout logic in the SDK.
- Runtime modules may import only the standard library and relative `saucepan_sdk` modules.

## Source boundaries

Keep the SDK excluded from the published Rust crate. Keep SDK development governance scoped by `sdk/python/zpp.toml`; do not impose Python BDD/TDD settings on the Rust repository root.
