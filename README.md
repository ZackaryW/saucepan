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
```

### Source precedence

When `install` is called, sources are tried in order: `github` then `customgit`. The first successful fetch wins.

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

### `bucket add|remove|list`

Manage registered bucket sources. Accepts a local file path or `file://` URL.

```
saucepan . bucket add /path/to/bucket.json
saucepan . bucket list
saucepan . bucket list --json
saucepan . bucket remove /path/to/bucket.json
```

JSON output for `bucket list`: one `{"url":"..."}` object per line.

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

An array of stubs. The `url` field is informational; saucepan does not fetch from it during search.

```json
[
  { "name": "my-tool",    "version": "1.2.0", "url": "https://github.com/owner/my-tool" },
  { "name": "other-tool", "version": "0.9.0", "url": "https://github.com/owner/other-tool" }
]
```

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
```

Directory names under `github/` use `--` as a separator for `/` so `owner/repo` and `owner_repo` never collide.

GitHub index entries written by newer versions may also contain optional `reference` and `resolved_commit` fields. Existing entries without these fields remain valid.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Not found |
| 2 | Source error (clone / fetch failure) |
| 3 | Config error (missing or invalid `saucepan.toml`) |
| 4 | Conflict (same name installed from a different source type) |
| 5 | Unexpected internal error |

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
