# Saucepan component contracts

This reference describes the 0.5.x central-store API. Verify it against the checked-out revision before changing a public contract.

## Core and CLI

One user-level store owns encrypted settings, source records, and app-specific touched-entry views. An ordinary caller supplies `--app`; an authoritative caller supplies a stable registration token through `--marker`. Filters select the app's subindex; they are not acquisition permissions. Settings are centrally registered/configured, with no workspace TOML.

Operational commands emit one JSON value on success: `init`, `register`, `configure`, `acquire`, `view`, `verify`, `path`, `mirror`, `history`, `snapshot`, and `shared-executable`. Core failures currently exit 1; command-line usage failures exit 2. Do not apply the former workspace API's error-code categories or NDJSON parser.

Recipes describe Git origins and refs, HTTP(S) file/ZIP downloads, or local paths. Folder selection does not split source identity. Paths and history come from the core. See [the central-store guide](../../../docs/central-source-store.md) for exact JSON examples and behavior.

## TypeScript SDK

`sdk/typescript` is an ESM package for Node.js 20+, with emitted TypeScript declarations and no runtime dependencies. `Saucepan` accepts an optional executable, app or marker/token context, timeout, output buffer limit, and explicit test root/key pair. It launches the CLI with argument arrays and no shell.

Each request owns its temporary JSON input files and removes them after success or failure. Registration tokens stay stable; neither settings nor scoped views are cached in the adapter. `SaucepanError` retains exit status, process diagnostics, and captured output without copying Node's command-bearing error message. `path()` and `history()` preserve nullable CLI results. Token and view format versions are checked.

See [the TypeScript README](../../../sdk/typescript/README.md) for the complete API and package installation.

## Shell SDK

`sdk/shell/saucepan.sh` is sourced into a POSIX shell. Named functions and `saucepan_call` forward quoted arguments to the executable. Environment variables select the executable and caller/test context. The library preserves raw JSON stdout, stderr, and core exit status; wrapper configuration errors exit 64. Per-call subshells keep variables and shell state out of the caller.

The caller owns JSON files and parsing. No Node.js, Python, or jq is required at runtime. Windows execution requires a POSIX shell such as Git Bash or an independently configured WSL environment. See [the shell README](../../../sdk/shell/README.md).

## Binary and legacy boundaries

All three adapters default to the user-level `.saucepan/bin` executable and allow an explicit path. Binary acquisition is external; no adapter downloads or builds it during runtime. Explicit test root/key pairs never become production fallbacks. Native keyring behavior belongs to the core.

Python 0.5.0 exposes a synchronous `Saucepan` client with the same central-store operations. It requires Python 3.9+, uses only the standard library, and accepts app, token/marker, timeout, and explicit test root/key options. It parses one JSON result, checks token/view versions, preserves nullable results and process diagnostics, and cleans up private request files. The old `Workspace`/entity API and workspace error categories are removed; callers must migrate. See the [Python README](../../../sdk/python/README.md).

SDK packages remain outside the published Rust crate. Source references under `src2` and `src3` are local-only and are never integration targets.
