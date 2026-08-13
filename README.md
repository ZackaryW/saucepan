# saucepan

Saucepan is a command-line artifact resolver for versioned `sauce.json`
manifests. Give it a workspace and a target; it finds the target through local,
GitHub, or custom-Git sources, records the installed manifest, and exposes
stable JSON and exit-code contracts for other tools.

## Install

```sh
cargo install saucepan
```

Saucepan requires `git` (or `gh`) for remote sources. The `search` command also
requires `jq`. See [Configuration](docs/configuration.md) for authentication and
executable overrides.

## Quick start

Create a workspace with at least one enabled source:

```sh
mkdir my-workspace
cd my-workspace

cat > saucepan.toml <<'EOF'
[github]
EOF

saucepan . install owner/my-tool
saucepan . list
saucepan . path my-tool
```

Commands identify an installed sauce by the `name` inside its manifest, which
can differ from the repository target used during installation.

## How resolution works

An installation has two distinct phases:

```text
enabled source finds target        fetched target supplies manifest
───────────────────────────        ─────────────────────────────────
local installed entry              1. repository-root sauce.json
        ↓                           2. registered central indexes
GitHub repository          ──────▶  3. not found (exit 1)
        ↓
custom-Git repository
```

The repository's own manifest always wins. A central index can supply a
manifest and Git ref only when the repository has no root manifest. Installed
entries retain both repository provenance and the source of their manifest.

Read [Central indexes](docs/central-indexes.md) for registration, pinning,
target matching, precedence, and update behavior.

## Common commands

```sh
# Install a default branch, tag, branch, or commit
saucepan . install owner/my-tool
saucepan . install owner/my-tool --ref v1.2.0

# Inspect and update installed state
saucepan . list --json
saucepan . update my-tool
saucepan . cat sauce my-tool
saucepan . uninstall my-tool

# Register and query a central index
saucepan . bucket add owner/central-index --ref v1.0.0
saucepan . bucket refresh owner/central-index
saucepan . search '.name | startswith("my-")'
```

The complete command contract is in the [CLI reference](docs/cli-reference.md).

## Automation and SDK use

`list --json` and `bucket list --json` emit newline-delimited JSON. The `cat`
family emits complete JSON documents, `search` emits raw jq-selected JSON, and
`path` emits a bare filesystem path. Saucepan uses stable exit categories:

| Code | Meaning |
|---:|---|
| 0 | Success |
| 1 | Not found |
| 2 | Git/gh source failure |
| 3 | Missing or invalid configuration |
| 4 | Installed-name conflict with another origin |
| 5 | Unexpected internal failure |

Python callers can use the standard-library-only
[Saucepan SDK](sdk/python/README.md), which drives an independently installed
Saucepan executable and maps these exits to typed exceptions.

## Documentation

- [Getting started](docs/getting-started.md)
- [Configuration](docs/configuration.md)
- [CLI reference](docs/cli-reference.md)
- [Central indexes](docs/central-indexes.md)
- [Artifact formats and workspace layout](docs/artifact-formats.md)
- [Central-index example](docs/examples/central-index/README.md)

## License

MIT
