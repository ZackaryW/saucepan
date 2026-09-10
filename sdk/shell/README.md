# Saucepan shell SDK

Source `saucepan.sh` from a POSIX-compatible shell (sh, Bash, or zsh in compatible mode). On Windows use Git Bash or another POSIX environment. Native cmd.exe/PowerShell are not shell SDK targets.

The library wraps the **0.5.x central-store CLI**. It has no Node, Python, jq, or downloader dependency. JSON is passed through unchanged; parse it with a tool of your choice when needed. Supply the executable externally or place it at `$HOME/.saucepan/bin/saucepan[.exe]`.

```sh
. /path/to/saucepan/sdk/shell/saucepan.sh
export SAUCEPAN_BIN=/path/to/saucepan   # optional; quote paths containing spaces
# saucepan_init                       # first user-store initialization only
saucepan_register my-app > .saucepanhash  # once per app
export SAUCEPAN_MARKER="$PWD/.saucepanhash"

cat > recipe.json <<'JSON'
{"source":{"provider":"local","path":"./assets"}}
JSON
saucepan_acquire recipe.json
saucepan_view > view.json
saucepan_verify view.json
```

Create `assets` with the content you want first. Settings and filters are JSON inputs to registration/configuration; they are persisted in the central encrypted index, not reloaded from local settings files on each operation.

| Function | Arguments after the function name |
| --- | --- |
| `saucepan_init` | none |
| `saucepan_register` | `APP [--settings FILE] [--filters FILE]` |
| `saucepan_configure` | `[--settings FILE] [--filters FILE]` |
| `saucepan_acquire` | `RECIPE_FILE` |
| `saucepan_view` | none |
| `saucepan_verify` | `VIEW_FILE` |
| `saucepan_path` | `ARTIFACT_ID` |
| `saucepan_mirror` | `ARTIFACT_ID DESTINATION` |
| `saucepan_history` | `SOURCE_ID` |
| `saucepan_snapshot` | `SOURCE_ID SNAPSHOT_ID [--folder PATH]` |
| `saucepan_shared_executable` | none |

`saucepan_call COMMAND ...` is the direct pass-through entry point. Quote every caller argument containing spaces or shell metacharacters. Use the CLI's `--` separator before positional values beginning with a hyphen, for example `saucepan_register -- -my-app`.

Configuration is read per call:

| Variable | Meaning |
| --- | --- |
| `SAUCEPAN_BIN` | Explicit compatible executable; otherwise use the user-level bin path |
| `SAUCEPAN_APP` | Registered app name for ordinary calls, or an optional identity paired with a marker |
| `SAUCEPAN_MARKER` | Caller-owned stable token JSON file; implies authoritative mode |
| `SAUCEPAN_AUTHORITATIVE` | `1` requires proof; empty or `0` leaves the CLI's default |
| `SAUCEPAN_TEST_ROOT`, `SAUCEPAN_TEST_KEY` | Explicit test store, requiring both a nonempty path and a 64-hex-character key |

An incomplete test configuration fails with status 64 instead of using the real user store. Wrapper configuration errors use stderr/status 64. Otherwise stdout, stderr, and the executable's exit status are preserved. CLI launch failures use the shell's normal status. The library does not change the caller's shell options, variables, cwd, or traps; invocation-local variables stay in a subshell. It never uses `eval` or assembles a command string.

The caller creates and owns all recipe, settings, view, and marker files. No JSON parser, implicit registration, token refresh, cache, or permission controller is added. Core behavior and platform limitations are documented in the repository's `docs/central-source-store.md`.

With safe Git symlink export in the supplied CLI, committed relative links become ordinary copied content before folder selection. Targets can be elsewhere in the same complete source tree. Unsafe/broken/cyclic links fail through the normal CLI status; no shell option or symlink privilege is required. Local filesystem links and URL ZIP symlink entries remain unsupported.

To validate from the repository root (Node.js is used only by the test harness):

```sh
cargo build --locked
node --test sdk/shell/tests/run.mjs
```

Use `SAUCEPAN_TEST_BINARY` and `SAUCEPAN_TEST_SHELL` to select the real executable and shell. The tests exercise every wrapper using isolated custom stores.
