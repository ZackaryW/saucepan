# Getting started

## 1. Install the CLI

```sh
cargo install saucepan
```

Remote sources require `git` or `gh`. Searching registered buckets requires
`jq`.

## 2. Create a workspace

Every command takes the workspace root as its first argument. The root must
contain a readable `saucepan.toml`.

```sh
mkdir my-workspace
cd my-workspace

cat > saucepan.toml <<'EOF'
[github]
EOF
```

An empty `saucepan.toml` is valid for read-only operations, but installation
requires at least one of `[local]`, `[github]`, or `[customgit]`.

## 3. Install and inspect a sauce

```sh
saucepan . install owner/my-tool
saucepan . list
saucepan . list --json
```

Saucepan reads `sauce.json` from the repository root by default. A typical
manifest looks like this:

```json
{
  "name": "my-tool",
  "version": "1.2.0",
  "description": "A useful command-line tool"
}
```

After installation, use the manifest name—not necessarily the repository
target—to address the sauce:

```sh
saucepan . path my-tool
saucepan . cat sauce my-tool
saucepan . update my-tool
```

## 4. Select a revision when needed

`--ref` accepts a branch, tag, or commit. Saucepan records both the requested
ref and its resolved commit.

```sh
saucepan . install owner/my-tool --ref v1.2.0
```

Updates resolve the stored ref again: branches can advance, while tags and
commit SHAs remain at the commit they resolve to.

## Next steps

- Configure private repositories or custom Git in [Configuration](configuration.md).
- Supply manifests for repositories you do not control with
  [Central indexes](central-indexes.md).
- Integrate Saucepan into another tool with the [CLI reference](cli-reference.md)
  or [Python SDK](../sdk/python/README.md).
