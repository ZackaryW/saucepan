# Design

## Context

See proposal.md (Why). The current acceptance surface:

| Script | Fake provider | Drives the manager by | In hosted checks |
| --- | --- | --- | --- |
| `scripts/acceptance.py` | own `BaseHTTPRequestHandler` | literal CLI argv | no |
| `scripts/lifecycle_acceptance.py` | own handler | literal CLI argv | no |
| `scripts/test_setup.py` | own handler plus installer downloads | CLI and installer scripts | Windows |
| `sdk/contracts/fixtures/setup_flow.py` | own handler | SDK adapter | SDK checks |
| `tests/support/mod.rs` | Rust in-process server | Rust API | yes |

The Python SDK (`sdk/python`, stdlib-only, Python 3.12+) maps every public resource operation to the CLI. Its `CliTransport._invoke` is already subclassed in `sdk/contracts/fixtures/resource_flow.py` to prepend `--test-github-api`. It deliberately omits legacy administration commands (`activate`, `deactivate`, `install-shim`, `replace-shim`, `register-app`). The CLI exposes hidden `--test-now` and `--test-github-api` only; `AcquireOptions` already has `test_github_raw`, `test_git_raw` and `test_gh` fields that only Rust tests can set.

## Goals / Non-Goals

**Goals:**
- One Python provider fixture, one sandbox and one watchdog shared by every real-build acceptance scenario and by the setup and SDK fixture suites.
- Scenarios that read as behavior, with contract spelling owned by the SDK or one harness adapter.
- Runs that are bounded, diagnosable and selectable, and that report the launch environment.

**Non-Goals:**
- Replacing Rust integration tests or `tests/support`; they stay in-process and in Rust.
- Porting `scripts/access_acceptance.py` or native-keyring checks; they keep their own entry points.
- Changing the installer scripts or release packaging.

## Decisions

### Python package `acceptance/` run as `python -m acceptance`
Python matches the SDK, the setup tests and the installer flows, and needs no new toolchain. The runner uses `unittest` with a custom result collector, so no third-party dependency is added. Alternative: a Rust test crate. Rejected because the SDK and the installer scripts are the surfaces under test, and Rust cannot use the Python SDK.

Layout:
- `acceptance/forge.py`: the provider fixture.
- `acceptance/sandbox.py`: run home, per-scenario root, clock, SDK client and admin adapter.
- `acceptance/watchdog.py`: deadlines, process-tree snapshots and kills.
- `acceptance/builds.py`: debug manager plus labeled release builds, each built once per run.
- `acceptance/report.py`: the environment and outcome report.
- `acceptance/scenarios/test_*.py`: the scenarios.
- `acceptance/__main__.py`: argument parsing and the runner.

### Forge: declarative state over a threading HTTP server
The forge has a `Forge` object holding repositories, each with releases (id, tag, prerelease flag, assets by name to bytes), refs to commits, files by commit and path (from which raw files and zipballs are derived), and installer downloads by tag and file. One `ThreadingHTTPServer` on `127.0.0.1:0` serves these routes:
- GitHub API: `/repos/{o}/{r}/releases/latest`, `/releases/tags/{tag}`, `/releases/assets/{id}`, `/commits/{ref}` (a SHA body for `application/vnd.github.sha`), `/zipball/{commit}` (single root folder `{o}-{r}-{short}`).
- GitHub raw: `/{o}/{r}/{commit}/{path}`.
- GitLab raw `/{repo}/-/raw/{ref}/{path}` and Gitea raw `/{repo}/raw/{ref}/{path}`, served from separate base paths so one forge plays both platforms.
- Installer downloads: `/releases/{tag}/{file}`.

Each request is logged as method, path and authorization value. `forge.fail = True` returns 503 for discovery. Release asset `browser_download_url` points at a route that returns 410 Gone, so a regression to browser URLs fails loudly. Alternative: keep per-script handlers with a shared helper. Rejected because route knowledge would still be duplicated.

### Sandbox owns environment, root and clock
The runner creates one run directory `target/acceptance/<run-id>/` with `home/`. Before any scenario it sets `HOME`, `USERPROFILE`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `XDG_DATA_HOME`, `APPDATA` and `LOCALAPPDATA` in `os.environ` to paths under it, so the SDK, the manager, shims and consumers all inherit them. Each `Sandbox` then points them at its own `home/` for the scenario's duration, so a default root one scenario creates is invisible to the next. A test asserts that the default root (no `--root`) resolves inside the sandbox home. Each scenario gets `Sandbox(run, name)`, which provides:
- a fresh `root` with `init` already run;
- `clock` (starting at 100, with `advance(days=…)`);
- `client`, an SDK `Client` whose transport subclass prepends `--test-now <clock>`, `--test-github-api`, `--test-github-raw`, `--test-git-raw` and optionally `--test-gh`;
- `admin`, the typed adapter for SDK-less commands, with methods `activate`, `deactivate`, `install_shim`, `replace_shim` and `register_app` that are the only place spelling their argv;
- `forge`.

Alternative: pass `env=` per subprocess. Rejected because the SDK transport does not take an env, and process-wide isolation also covers shims launched by the manager.

### Hidden CLI test options validated at parse time
`--test-github-raw`, `--test-git-raw` and `--test-gh` sit beside the existing hidden options and flow into `AcquireOptions`. URL overrides are checked as loopback when the CLI builds options and map to `invalid_request`. Today a non-loopback value fails later with an untyped error. `uses_test_endpoints()` already prevents the real `gh` from running when any override is present.

### Watchdog: deadline per invocation, snapshot, then kill the tree
Each SDK client gets `timeout = deadline + 30 s`, so the watchdog fires first. The watchdog wraps each scenario step. On expiry a timer thread:
1. snapshots processes that descend from the runner: `Get-CimInstance Win32_Process` on Windows, `ps -eo pid,ppid,etime,args` elsewhere;
2. records the step's command and elapsed time;
3. kills the descendants with `taskkill /T /F` or `os.killpg`;
4. fails the scenario with the snapshot attached.

The default deadline is 120 s per call, and steps known to be long declare a larger one. Alternative: rely on the SDK timeout alone. Rejected because it reports nothing about where the process was blocked.

### Builds once per run
`builds.manager()` runs `cargo build --bins` once and returns the debug manager and shim. `builds.release(label)` runs `cargo build --release --bins` with `SAUCEPAN_BUILD_ID=label`, then copies the executable into the run directory and packs `saucepan.zip`. Results are memoized per label, and only scenarios that need distinct identities request release builds.

### Scenario map

| Scenario (tag) | Replaces |
| --- | --- |
| `release_lifecycle` (lifecycle) | `acceptance.py` bootstrap, update, busy launcher, failed candidate and reclaim; `lifecycle_acceptance.py` shared app, shims, deactivate, replace-shim, uninstall, reconcile and generic install |
| `freshness` (freshness) | `acceptance.py` cached update, expiry, provider outage and stale fallback |
| `retention` (retention) | `acceptance.py` eleven-entry retention and identical reuse |
| `platform_sources` (platform-sources) | new: `github_raw`, `github_source`, `git_raw` and borrowed fixture `gh` token, through the real binary |
| `cli_smoke` (cli) | new: help, `--root` precedence, error envelope and exit codes, default root inside sandbox home |
| setup suites (setup) | `scripts/test_setup.py` and `setup_flow.py` keep their tests but use `Forge` |

### Report and launch environment
The report records:
- `platform` (`sys.platform`, machine);
- `launch_environment`: `SAUCEPAN_ACCEPTANCE_SHELL` if set, otherwise `git-bash` when `MSYSTEM` is set, `windows-native` on Windows and `posix-shell` elsewhere;
- `git` (`shutil.which('git')`), because the Git Bash hang depended on which git binary was resolved;
- build labels and digests;
- per-scenario outcome (`passed`, `failed`, `skipped`, `not_selected`) with diagnostics.

It is written to `target/acceptance/<run-id>/report.json` and `target/acceptance/latest.json`.

### Hosted checks
`core-platforms.yml` gains acceptance steps:
- the Windows job runs `python -m acceptance` once with `shell: pwsh` and once with `shell: bash` (Git Bash), setting `SAUCEPAN_ACCEPTANCE_SHELL`;
- the Linux job runs it with `bash`.

Reports are uploaded as artifacts. Triggers stay manual or reusable-workflow only.

## Risks / Trade-offs

- [Process-wide env mutation could leak into the runner's own tools (cargo, git)] → Builds run before the sandbox env is applied, and cargo receives the original environment explicitly.
- [The SDK hides argv, so CLI regressions could slip] → The `cli_smoke` scenario keeps literal-argv checks for parsing, roots, errors and exit codes.
- [Release builds dominate run time (two relinks)] → Only the lifecycle scenario requests them; other scenarios use the debug build.
- [Process-tree snapshots differ by OS and need PowerShell on Windows] → If the snapshot command fails, the failure report says so and the kill still runs.
- [Moving `setup_flow.py` onto the forge couples SDK contract fixtures to `acceptance/`] → The forge is a single stdlib module imported by path. SDK package verification already runs from the repository checkout.

## Migration Plan

1. Land the CLI test options and the harness with all scenarios, then confirm parity with the old scripts on Windows.
2. Switch `test_setup.py` and `setup_flow.py` to the forge.
3. Delete `scripts/acceptance.py` and `scripts/lifecycle_acceptance.py`, and update docs that name them.
4. Add the hosted acceptance steps.

Rollback is reverting the change; the old scripts remain available in history.
