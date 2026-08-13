# Configuration

Saucepan loads `<root>/saucepan.toml` for every command. A missing, unreadable,
or invalid file is a configuration error (exit 3). An empty file is valid and
enables no artifact source.

## Complete example

```toml
# Optional jq executable used by `search`.
jq = "/usr/local/bin/jq"

# Treat already-installed index entries as the local source.
[local]

# Fetch direct GitHub targets.
[github]
binary = "git"          # "git" (default) or "gh"
token = ""              # used by gh; raw git uses native credentials
ssl_key = ""            # optional
manifest = "sauce.json" # default

# Fetch names relative to a custom Git base URL.
[customgit]
url = "https://git.example.com/repos"
binary = "git"
token = ""              # optional
ssl_key = ""            # optional
manifest = "sauce.json" # default

# Fetch standalone repository-target central indexes.
[index]
binary = "git"          # "git" (default) or "gh"
token = ""              # used by gh
ssl_key = ""            # optional
```

Only sections that are present are enabled. During installation Saucepan:

1. checks the local installed index when `[local]` is enabled;
2. tries `[github]` when present;
3. falls back to `[customgit]` when present.

If all enabled remote sources miss, installation returns not found. If none
succeeds and at least one fails operationally, it returns a source error while
preserving compact context for the attempted sources.

## GitHub authentication

With `binary = "gh"`, a configured token is passed to GitHub CLI through
`GITHUB_TOKEN`.

With `binary = "git"`, Saucepan delegates authentication to native Git
configuration, Git Credential Manager, or an SSH agent. A configured token is
ignored with a warning; Saucepan does not put tokens in clone URLs or generate
askpass scripts.

## Central-index authentication

Standalone `bucket add --ref`, `bucket refresh`, `search`, and `cat bucket`
operations use `[index]`, defaulting to unauthenticated raw Git when the section
is absent.

During `install` or `update`, the central-index resolution link reuses the
authentication of the GitHub or custom-Git source already in flight.

See [Central indexes](central-indexes.md) for the distinction between registered
index state and fetched index state.
