# saucepan

Composable multi-source artifact resolver. Pass a folder path; if it contains a `saucepan.toml`, saucepan can install, update, and query versioned manifests from local indexes, GitHub repos, or any custom Git server.

Designed as a middleware component — every read command supports `--json` for machine-readable output and the `cat` command exposes raw state as JSON so callers can integrate without screen-scraping.

## Installation

```
cargo install saucepan
```

Requires `git` (or `gh`) on `PATH` for GitHub/custom-git sources, and `jq` on `PATH` for `search`.

## Quick start

```
mkdir my-workspace
cd my-workspace
cat > saucepan.toml << 'EOF'
[local]

[github]
EOF

saucepan . list
saucepan . install owner/repo
saucepan . list --json
```

## Configuration — `saucepan.toml`

```toml
# Optional: path to a jq binary if not on PATH
jq = "/usr/local/bin/jq"

# Enable local source (entries already in the index are visible)
[local]

# Enable GitHub source
[github]
binary = "git"          # or "gh" for GitHub CLI
token  = ""             # optional with gh; raw git uses native credentials
manifest = "sauce.json" # default; name of the manifest file in each repo

# Enable a custom Git server
[customgit]
url    = "https://git.example.com/repos"
binary = "git"
manifest = "sauce.json"

# Optional: authentication for standalone bucket commands (bucket add/refresh,
# search, cat bucket) when they need to fetch a repository-target central
# index and there is no install/update already in flight to inherit auth from.
[index]
binary = "git"   # or "gh"
token  = ""      # optional with gh
ssl_key = ""     # optional
```

### Source precedence

When `install` is called, sources are tried in order: `github` then `customgit`. The first successful fetch wins.

### Manifest resolution chain

Once a target is fetched, saucepan resolves its manifest through an ordered chain rather than a single existence check:

1. **The repository's own manifest.** If the fetched target's root carries the configured manifest file (`sauce.json` by default), it is used. A repository that declares its own manifest always wins — no index can override it.
2. **Registered central indexes**, consulted only when step 1 finds nothing. Each registered index (see `bucket add` below) is checked, in registration order, for an entry whose `url` matches the fetch target exactly and that supplies a full manifest and a ref (see "`bucket.json` — marketplace index" under Artifact formats, below). The first such entry found wins; if a later index also describes the same target, saucepan warns that it was shadowed.
3. **Not found**, if no link supplies a manifest — the same not-found result (exit code 1) saucepan has always returned for a manifest-less target.

The chain is a list, not a hard-coded pair of checks, so a future link can be appended after the central-index link without changing the precedence or outcome of the links above.

An index that cannot be fetched or read is warned about and skipped; it never fails an install or update that another link can satisfy. With no index registered, the chain has exactly one effective link and behavior is identical to saucepan versions before this feature existed.

An installed entry records which link supplied its manifest (see `manifest_source` below), so `cat sauce`/`cat index`/`list --json` can always answer "did this come from the repository or from an index."

## Commands

```
saucepan <root> [--json] <command>
```

### `install <target> [--ref <ref>]`

Fetch and record a sauce. For `github`, `<target>` may be `owner/repo`, an explicit Git URL, or a filesystem path. Raw `git` expands a strict `owner/repo` target to `https://github.com/owner/repo.git`; `gh` accepts the slug directly. For `customgit`, `<target>` is appended to the configured base URL.

```
saucepan . install owner/my-tool
saucepan . install owner/my-tool --ref v1.2.0
```

`--ref` accepts a branch, tag, or commit. Saucepan records both the requested ref and resolved commit. Without `--ref`, installation follows the repository default branch as before.

After installation, commands address the sauce by the `name` declared in its manifest, which may differ from the repository target. Exit 1 if no source can satisfy the target. Exit 4 if the manifest name is already installed from a different source type or origin (uninstall first).

### `update <name>`

Re-clone and refresh the index entry. Local sauces do not support update.

```
saucepan . update my-tool
```

An installation with a stored ref resolves the same ref again: branches can advance, tags resolve to their tagged commit, and commit SHAs remain pinned.

### `path <name>`

Print the on-disk directory path of an installed sauce. Designed for shell composition — outputs a bare path with no decoration.

```
saucepan . path my-tool
```

Exit 1 if the sauce is not installed. The path is always the cloned repo root, so any file inside it can be reached with normal path arithmetic.

### `uninstall <name>`

Remove an installed sauce by its manifest name.

```
saucepan . uninstall my-tool
```

GitHub and custom-Git checkouts are deleted only from their Saucepan-managed workspace directories. Local entries are removed from the index without deleting their source paths. Exit 1 if the sauce is not installed.

### Private repositories

- With `binary = "gh"`, an optional configured `token` is supplied as `GITHUB_TOKEN`.
- With `binary = "git"`, authentication comes from native Git Credential Manager, an SSH agent, or existing Git configuration. A configured `token` is ignored with a warning for backward compatibility.

Saucepan does not place tokens in clone URLs, generate askpass scripts, or modify global Git configuration.

### `list [--json]`

Show installed sauces.

```
saucepan . list
saucepan . list --json
```

Human output: `name version — description`
JSON output: one `IndexEntry` object per line (NDJSON).

### `search <jq-filter>`

Query all registered buckets using a jq filter over their stub arrays.

```
saucepan . search '.name | startswith("my-")'
saucepan . search '.version == "2.0.0"'
```

Output is the raw jq result — each matching stub on its own line as compact JSON.

### `bucket add|refresh|remove|list`

Manage registered bucket sources — including central indexes that supply manifests through the resolution chain above. A bucket can be registered three ways:

- a local file path,
- a `file://` URL,
- or a **repository target** — anything git/gh can clone: an `owner/repo` slug, a full Git URL, or a local path to a repository (as opposed to a path to a `bucket.json` file directly). A repository-target index is fetched through the same clone/pull/authentication path used for sauces, at `<root>/indexes/<name>/`, and its `bucket.json` is read from the repository root.

```
saucepan . bucket add /path/to/bucket.json
saucepan . bucket add owner/central-index
saucepan . bucket add owner/central-index --ref v1.2.0
saucepan . bucket refresh owner/central-index
saucepan . bucket list
saucepan . bucket list --json
saucepan . bucket remove owner/central-index
```

`--ref` pins a repository-target index to an explicit branch, tag, or commit. Giving `--ref` makes `bucket add` fetch synchronously and persist the resolved commit to `buckets.json` immediately; if that ref cannot be resolved, the registration is rolled back rather than left half-registered. Without `--ref`, `bucket add` is a pure, local, offline operation exactly as before this feature existed — nothing is fetched at registration time.

`bucket refresh <url>` re-fetches an already-registered index (of any kind) and updates its recorded resolved commit — the way to pick up new state for an index registered without a pin, or to re-sync one whose pinned ref has moved. Plain reads (`search`, `cat bucket`, and the manifest resolution chain itself) never fetch on your behalf in a way that mutates `buckets.json`; only `bucket add --ref` and `bucket refresh` do.

Fetching a registered index for `search`, `cat bucket`, `bucket add --ref`, or `bucket refresh` uses the `[index]` config section for authentication (see Configuration, above), defaulting to plain unauthenticated `git` when that section is absent. During `install`/`update`, the in-flight central-index chain link instead reuses the primary source's own authentication, since a fetch is already underway there.

JSON output for `bucket list`: one `BucketEntry` object per line — `{"url":"..."}`, plus `reference` and/or `resolved_commit` when the index is pinned or has been fetched.

### `cat <target>`

Emit raw JSON for any piece of state. Always outputs JSON — no human-readable fallback.

| Target | Output |
|---|---|
| `cat index` | Full `.saucepan/index.json` as a pretty JSON array |
| `cat buckets` | Full `.saucepan/buckets.json` as a pretty JSON array |
| `cat sauce <name>` | Single `IndexEntry` object for the named sauce |
| `cat bucket <url>` | Contents of a `bucket.json` at the given path or `file://` URL |

```
saucepan . cat index
saucepan . cat sauce owner/my-tool
saucepan . cat bucket /path/to/bucket.json
```

## Artifact formats

### `sauce.json` — manifest

Required fields: `name`, `version`, `description`. All additional fields are preserved as-is.

```json
{
  "name": "my-tool",
  "version": "1.2.0",
  "description": "A short description",
  "custom_field": "preserved"
}
```

### `bucket.json` — marketplace index

An array of stubs. `name`, `version`, and `url` are required on every entry — this is a normative compatibility guarantee, not just a convention: an index written for a newer saucepan must still parse on an older binary, and dropping any of these three (all non-optional) would break that. An entry omitting any of them is rejected. All other fields are flattened onto the entry (an `extra` map, not a nested object) and preserved verbatim, so an entry can carry a full manifest document or arbitrary consumer data alongside the required three.

```json
[
  { "name": "my-tool",    "version": "1.2.0", "url": "https://github.com/owner/my-tool" },
  { "name": "other-tool", "version": "0.9.0", "url": "https://github.com/owner/other-tool", "note": "curated by zush" }
]
```

For `search` and `cat bucket`, `url` remains purely descriptive. For the manifest resolution chain, `url` doubles as a **matching key**: an entry is considered as a manifest source for a target only when `url` equals the exact string saucepan fetched — for `github`, that is the `install`/`update` target argument verbatim (an `owner/repo` slug or full URL, whichever was used); for `customgit`, the full constructed URL (`<customgit.url>/<name>`).

An entry that supplies a manifest **must** also supply the ref it describes, using two reserved extra fields:

- `manifest`: the complete `sauce.json` document (`name`, `version`, `description`, plus anything else) that should apply to the matching target.
- `ref`: a branch, tag, or commit. Saucepan checks this ref out on the target's own clone before recording it, so the version an index asserts can never disagree with the working tree.

```json
[
  {
    "name": "third-party-index-entry",
    "version": "1.0.0",
    "url": "owner/plain-script-repo",
    "ref": "v2.0.0",
    "manifest": { "name": "widget-cli", "version": "2.0.0", "description": "Curated by an index" }
  }
]
```

An entry that supplies `manifest` without `ref` is rejected — saucepan warns and continues the chain rather than installing with a version that could drift from the checked-out tree.

The stub's own `name`/`version` are independent of the `manifest` it carries — they identify the entry for `search`/`cat bucket` purposes, while `manifest.name`/`manifest.version` become the installed sauce's actual identity once the chain resolves it. They are commonly kept in sync but nothing requires it.

## Workspace layout

```
<root>/
  saucepan.toml
  .saucepan/
    index.json      ← installed sauce records
    buckets.json    ← registered bucket URLs
  github/
    owner--repo/    ← cloned GitHub repos  (/ maps to --)
  customgit/
    name/           ← cloned custom-git repos
  indexes/
    name/           ← cloned repository-target central indexes
```

Directory names under `github/` use `--` as a separator for `/` so `owner/repo` and `owner_repo` never collide. `indexes/` holds one clone per repository-target index registered via `bucket add`; local-path and `file://` indexes are read directly and never land here.

GitHub index entries written by newer versions may also contain optional `reference` and `resolved_commit` fields. Existing entries without these fields remain valid.

Every `Github`/`Customgit` index entry also carries `manifest_source` — `{"kind":"repository"}` when the manifest came from the fetched target itself, or `{"kind":"index","index":"<registered index target>"}` when a central index supplied it. It is visible through `cat index`, `cat sauce <name>`, and `list --json`. Entries written before this field existed have no `manifest_source` key at all and deserialize as `{"kind":"repository"}` — the only manifest source that existed at the time. `Local` entries never carry this field: a local entry never passes through the resolution chain, so no chain link ever supplies its manifest.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Not found |
| 2 | Source error (clone / fetch failure) |
| 3 | Config error (missing or invalid `saucepan.toml`) |
| 4 | Conflict (same name installed from a different source type) |
| 5 | Unexpected internal error |

The mapping above, and the failure categories behind it, are unchanged by the manifest resolution chain: a target with no manifest anywhere in the chain still exits 1, and every other code still means exactly what it meant before.

**Behavior change for exit-1 feature detection only.** Before this feature, exit 1 on `install`/`update` reliably meant "this target carries no manifest." Now it means "no link in the manifest resolution chain — the target's own manifest, then registered central indexes — could supply one." A target that previously failed with exit 1 for lacking its own manifest may now succeed if a registered central index describes it. If you have relied on exit 1 as a feature-detection signal specifically for "this repository has no manifest," that signal is now conditioned on whichever indexes are registered in the workspace. Every operation that already succeeded before this change is unaffected: the target's own manifest, when present, still wins outright, and the failure category itself (not-found, exit 1) has not changed — only the set of targets that reach it has shrunk. This is the only observable behavior change in this feature; everything else described in this README is additive.

## Middleware usage

saucepan is designed to be called from other tools. The `--json` flag and `cat` subcommand exist specifically for this use case; exit codes are stable and suitable for shell conditionals.

Python callers can use the [`sdk/python`](sdk/python/README.md) object-oriented client over this existing CLI.

```bash
# Resolve the artifact path and use it directly
cp "$(saucepan /workspace path owner/my-tool)/build/output.bin" /usr/local/bin/

# Get the version of an installed sauce
saucepan /workspace cat sauce owner/my-tool 2>/dev/null | jq -r .sauce.version

# List all installed sauces as a JSON array
saucepan /workspace list --json | jq -s '.'

# Dump full index and bucket state
saucepan /workspace cat index
saucepan /workspace cat buckets
```

## License

MIT
