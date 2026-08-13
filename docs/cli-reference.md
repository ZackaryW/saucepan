# CLI reference

## Syntax

```text
saucepan [--json] <root> <command>
```

`<root>` is the directory containing `saucepan.toml`. `list --json` and
`bucket list --json` emit newline-delimited JSON. The `cat` family always emits
complete JSON documents, `search` emits jq-selected JSON values, and `path`
emits a bare filesystem path.

## Sauce lifecycle

### `install <target> [--ref <ref>]`

Fetch a target and record the resolved manifest. A GitHub target can be a strict
`owner/repo` slug, an explicit Git URL, or a filesystem path. A custom-Git name
is appended to the configured base URL.

`--ref` accepts a branch, tag, or commit. The installed identity is
`manifest.name`, not the target string.

### `update <name>`

Fetch the installed remote source again and replace its stored manifest and
resolved revision. GitHub entries with a requested ref resolve that ref again.
Index-sourced entries also consult the current registered indexes and may adopt
the current winning entry's manifest and ref. Local entries cannot be updated.

### `uninstall <name>`

Remove one installed entry. Saucepan deletes only GitHub and custom-Git
checkouts verified to be beneath their managed directories. Local source paths
are never deleted. A missing managed checkout is recoverable: the stale index
entry can still be removed.

### `list [--json]`

List installed sauces. Human output is `name version — description`; JSON output
is one complete installed entry per line.

### `path <name>`

Print the installed sauce's on-disk path without decoration.

## Bucket lifecycle and search

### `bucket add <target> [--ref <ref>]`

Register a local `bucket.json` path, `file://` URL, or repository target.
Without `--ref`, registration is local and performs no fetch. With `--ref`,
Saucepan fetches immediately, records the resolved commit, and rolls back the
registration if the ref cannot be resolved.

### `bucket refresh <target>`

Fetch an existing registration. Repository targets update their recorded
resolved commit. Refreshing a local path or `file://` URL confirms it remains
readable but has no commit to record.

### `bucket remove <target>`

Remove an existing registration without deleting its source repository or local
file.

### `bucket list [--json]`

List registrations in registration order. JSON output includes `url` and, when
present, `reference` and `resolved_commit`.

### `search <jq-filter>`

Apply a jq filter to entries from every registered bucket. Matching values are
written as compact JSON, one per line. An empty registry and a filter with no
matches both succeed with a clear human-readable result.

## Raw state

| Command | JSON output |
|---|---|
| `cat index` | Complete `.saucepan/index.json` array |
| `cat buckets` | Complete `.saucepan/buckets.json` array |
| `cat sauce <name>` | One installed entry |
| `cat bucket <target>` | Fetched `bucket.json` from a path, `file://` URL, or repository target |

When `<target>` matches a pinned registration, `cat bucket` uses its stored ref.
Reading a bucket never changes `buckets.json`.

## Exit codes

| Code | Category | Examples |
|---:|---|---|
| 0 | Success | Command completed |
| 1 | Not found | Unknown sauce, exhausted manifest chain, unknown bucket |
| 2 | Source error | Git/gh launch, authentication, fetch, checkout, or pull failure |
| 3 | Configuration error | Missing/invalid config or no enabled install source |
| 4 | Conflict | Another source type or origin already owns the manifest name |
| 5 | Internal error | Unexpected failure |
