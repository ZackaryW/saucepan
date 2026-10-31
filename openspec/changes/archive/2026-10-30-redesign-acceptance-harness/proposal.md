# Proposal

## Why

Real-build acceptance is spread across scripts that each hand-roll a fake GitHub server, call the CLI with literal argv and JSON, and choose their own roots. Making the platform-sources change pass needed six separate patches: the same asset-route fix in five fixtures, a full port of `scripts/acceptance.py` (broken since the flat-policy contract and not run in CI), and a fix for a setup test that opened the developer's real `~/.saucepan`. A Windows hang that appeared only when Saucepan was launched from Git Bash went unnoticed because no acceptance ran in more than one launch environment, and took an hour to locate because nothing reported where a stalled process was blocked.

## What Changes

- Add a Python acceptance harness under `acceptance/` with three shared parts:
  - **Forge**: one configurable loopback provider for GitHub releases, release assets, commit resolution, zipballs and raw files, plus GitLab and Gitea raw routes. It records requests and supports injected failures.
  - **Sandbox**: a fresh root per scenario, with `HOME`/`USERPROFILE` redirected to a temporary directory. It injects `--root`, the test clock and every provider endpoint, and exposes a controllable clock.
  - **Watchdog**: a per-call deadline that reports the process tree and the last command on a stall.
- Drive scenarios through the Python SDK, keeping a few CLI smoke checks for argv-level behavior.
- Replace `scripts/acceptance.py` and `scripts/lifecycle_acceptance.py` with independent, tagged scenarios that reuse binaries built once per run:
  - release lifecycle
  - freshness and stale fallback
  - retention and identical reuse
  - platform sources
  - CLI smoke
- Move the fake servers in `scripts/test_setup.py` and `sdk/contracts/fixtures/setup_flow.py` onto the shared forge.
- Add hidden, loopback-only `--test-github-raw`, `--test-git-raw` and `--test-gh` CLI options, so platform-source scenarios run through the real binary without contacting GitHub or the developer's `gh` login.
- Run acceptance in hosted checks on Windows (PowerShell and Git Bash) and Linux.
- **BREAKING** (developer tooling only): remove `scripts/acceptance.py` and `scripts/lifecycle_acceptance.py`; `python -m acceptance` replaces them.

## Capabilities

### New Capabilities
- `acceptance-harness`: how real-build acceptance isolates roots and home directories, provides provider fixtures, drives the manager, bounds and diagnoses stalls, selects scenarios, and reports the environments it actually ran in.

### Modified Capabilities
<!-- None: self-management, lifecycle-recovery and store-layout acceptance requirements are unchanged; the harness is how they are satisfied. -->

## Impact

- **New**: `acceptance/` package (forge, sandbox, watchdog, builds, scenarios, runner).
- **Removed**: `scripts/acceptance.py`, `scripts/lifecycle_acceptance.py`.
- **Changed**: `scripts/test_setup.py` and `sdk/contracts/fixtures/setup_flow.py` fixture servers; `src/cli/mod.rs` hidden test options; `.github/workflows/core-platforms.yml` acceptance job; developer documentation that names the old scripts.
- **Unchanged**: Rust integration tests keep their in-process `tests/support` server; `scripts/access_acceptance.py` and native-keyring checks stay separate.
- **Dependencies**: none added; the harness uses the standard library and the in-repo Python SDK.
