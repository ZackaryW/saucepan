# Real-build acceptance

[← Saucepan guide](../README.md#development)

`python -m acceptance` runs the built manager, shim and Python SDK against a loopback provider fixture. Nothing reaches the network, the developer's `gh` login, or the real `~/.saucepan`. It needs Python 3.12+, a Rust toolchain and no third-party Python packages.

## Run

```sh
python -m acceptance                        # every scenario
python -m acceptance --tag platform-sources # one tag
python -m acceptance --name freshness       # ids containing this text
python -m acceptance --list                 # scenarios and tags, nothing runs
```

The harness builds the debug manager and shim once. The `lifecycle` tag also builds two stamped release binaries (`lifecycle-a`, `lifecycle-b`), which takes about a minute.

| Tag | Covers |
| --- | --- |
| `lifecycle` | Bootstrap from a packaged release, update while busy, failed candidate, rollback, deactivate, shim replacement, uninstall with reconciliation, generic install |
| `freshness` | 24-hour discovery cache, expiry, provider outage, stale fallback |
| `retention` | Ten-revision retention across app claims, identical reuse |
| `platform-sources` | `github_raw`, `github_source` and `git_raw` with credentials |
| `cli` | Help, `--root` precedence, JSON error envelope, exit codes |
| `setup` | Managed setup and the PowerShell/POSIX installers (`scripts/test_setup.py`) |

Harness self-tests: `python -m unittest discover -s acceptance/tests -t .`

## Read the report

Each run writes `target/acceptance/<run-id>/report.json` and copies it to `target/acceptance/latest.json`. The report includes:

- `environment`: platform, the launch environment (`powershell`, `git-bash`, `windows-native` or `posix-shell`), and the `git` binary the manager will resolve. Set `SAUCEPAN_ACCEPTANCE_SHELL` to label the environment explicitly. `native_keyring` is always `not_run`.
- `builds`: label, path and SHA-256 of every binary built.
- `scenarios`: one entry per scenario, with `outcome` (`passed`, `failed`, `skipped` or `not_selected`), duration, and on failure the message and traceback.

Every manager, shim and SDK call has a deadline (120 s by default). On a stall, the scenario fails with a `stall` entry listing the command, the elapsed time and the processes still alive; those processes are then killed and the remaining scenarios still run.

Each scenario directory, `target/acceptance/<run-id>/scenarios/<name>/`, keeps its `root/`, `home/` and request payloads for inspection.

## Write a scenario

Add `acceptance/scenarios/test_<topic>.py` with a `Scenario` subclass and a `tags` set. Each test method gets a fresh sandbox with these attributes:

- `self.client`: the SDK client, with root, clock and endpoint overrides injected.
- `self.forge`: declare releases with `release()`, commits and refs with `commit()`, installer downloads with `installer()`. Inject outages with `fail`, hold discovery with `hold_discovery()`, and read requests with `requested()`.
- `self.clock`: call `advance(days=…)` instead of sleeping.
- `self.admin`: commands the SDK does not expose (`activate`, `deactivate`, `install_shim`, `replace_shim`, `register_app`).
- `self.sandbox.launch()` and `self.sandbox.background()`: run shims and consumers under the watchdog.

Do not start servers or build manager argv in a scenario. Extend the forge or the admin adapter, so a provider or CLI change is fixed in one place.

Hosted checks (`core-platforms.yml`) run acceptance on Windows from PowerShell and Git Bash and on macOS and Linux from bash. Each run uploads its own report.
