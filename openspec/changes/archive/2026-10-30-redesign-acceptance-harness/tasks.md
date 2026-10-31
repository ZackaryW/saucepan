# Tasks

## 1. Manager test endpoints (checkpoint: platform sources reachable through the CLI)

- [x] 1.1 Add hidden global `--test-github-raw`, `--test-git-raw` and `--test-gh` options flowing into `AcquireOptions`, and validate URL overrides as loopback at option build time with `invalid_request`; verify a Rust CLI test that a non-loopback override fails with `invalid_request` and that the options are absent from `--help`

## 2. Harness core (checkpoint: an empty scenario runs isolated and reports)

- [x] 2.1 Add `acceptance/forge.py` serving GitHub releases, asset API downloads, commit resolution, zipballs, raw files, GitLab and Gitea raw routes and installer downloads from declarative state, with a request log, a failure toggle and 410 for browser download URLs; verify with `acceptance/tests/test_forge.py` exercising each route over HTTP
- [x] 2.2 Add `acceptance/builds.py` (debug manager and shim once per run; labeled release builds memoized, packed as `saucepan.zip`) using the original environment for cargo; verify two requests for the same label build once
- [x] 2.3 Add `acceptance/sandbox.py`: run home with `HOME`/`USERPROFILE`/XDG/`APPDATA`/`LOCALAPPDATA` redirected, fresh initialized root per scenario, clock, SDK client with a transport injecting the clock and every endpoint override, and the typed `admin` adapter for `activate`, `deactivate`, `install_shim`, `replace_shim` and `register_app`; verify a test that a manager started without `--root` resolves its default root inside the sandbox home
- [x] 2.4 Add `acceptance/watchdog.py`: per-step deadlines, process-tree snapshot on Windows and POSIX, tree kill, and failure diagnostics with command, elapsed time and live processes; verify a test where a step running a sleeping child fails at its deadline with the child listed and the next step still runs
- [x] 2.5 Add `acceptance/report.py` and `acceptance/__main__.py` with tag and name selection, per-scenario outcomes (`passed`, `failed`, `skipped`, `not_selected`), launch-environment and git-binary detection, and `report.json` plus `latest.json`; verify `python -m acceptance --tag cli` reports the other scenarios as `not_selected`

## 3. Scenarios (checkpoint: parity with the retired scripts)

- [x] 3.1 Add the `cli_smoke` scenario (help without a store, `--root` precedence, JSON error envelope and exit codes, default root inside the sandbox home); verify it passes
- [x] 3.2 Add the `release_lifecycle` scenario covering bootstrap from a packaged release build, update to distinguishable bytes, busy launcher protected from reclaim, failed candidate preserved, rollback, shared-output app with installed shim, deactivate/reactivate, replace-shim, uninstall with reconcile and generic install; verify it passes on Windows and records build digests
- [x] 3.3 Add the `freshness` scenario (cached update within 24 h, expiry triggers discovery, provider outage fails without `allow_stale`, stale fallback with it) driven by the sandbox clock; verify it passes
- [x] 3.4 Add the `retention` scenario (two app claims, eleven-entry retention reclaiming the oldest unclaimed revision, update-claim moving off it, identical reuse); verify it passes
- [x] 3.5 Add the `platform_sources` scenario (`github_raw` pinned and by ref, `github_source` folder extraction, `git_raw` for GitLab and Gitea with a token file, fixture `gh` token observed on GitHub requests and no `gh` call without the override); verify it passes and the forge log shows only loopback requests

## 4. Consolidation (checkpoint: one provider fixture)

- [x] 4.1 Switch `scripts/test_setup.py` to the forge for release discovery, asset downloads and installer downloads; verify `python -m unittest discover -s scripts -p test_setup.py` passes
- [x] 4.2 Switch `sdk/contracts/fixtures/setup_flow.py` to the forge; verify the Python SDK suite and `npm test` pass
- [x] 4.3 Remove `scripts/acceptance.py` and `scripts/lifecycle_acceptance.py` and update references outside archived changes; verify `rg "acceptance.py|lifecycle_acceptance" --glob '!openspec/changes/archive/**'` finds only intended mentions

## 5. Hosted checks and documentation

- [x] 5.1 Add acceptance steps to `.github/workflows/core-platforms.yml` (Windows `pwsh` and Git Bash, Linux) that set `SAUCEPAN_ACCEPTANCE_SHELL` and upload reports; verify the workflow parses and steps reference `python -m acceptance`
- [x] 5.2 Document running, selecting and reading acceptance in `docs/acceptance.md` linked from the README development section; verify the documented commands run
- [x] 5.3 Run `python -m acceptance` from PowerShell and from Git Bash on Windows plus `cargo test`, clippy and fmt; verify all pass and record which environments were not run (Linux, macOS)

## Workflow follow-up

- Archive this change after review; commit synced specs with the archive.

## Verification notes

- 5.3 (Windows 11, x64): `python -m acceptance` passed 26/26 from Git Bash (162 s, git resolved to `ucrt64\bin\git.exe`) and 26/26 from PowerShell (163 s, git resolved to `cmd\git.exe`). Rust code is unchanged since `cargo test` (56 suites) passed with task 1.1; `cargo fmt --check` and `cargo clippy --all-targets -D warnings` pass.
- Not run: Linux and macOS acceptance (hosted job added, not executed here); native keyring custody.
- The setup suite (`scripts/test_setup.py`) is selectable as tag `setup` and replaces the separate hosted setup step.
