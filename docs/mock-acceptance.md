# Simulated installation

For the current coordinated lifecycle, use `uv run --no-project python scripts/lifecycle_acceptance.py`. It installs two real release builds in a fresh `target/lifecycle-acceptance/<uuid>/root`, covers update failure, rollback, launcher maintenance and recoverable uninstall, and compares every existing mock file's digest. See [lifecycle verification](lifecycle-verification.md). The older `mock` recipe below intentionally writes `mock`; it was not rerun for the lifecycle change. Do not run an old manager against newly written lifecycle metadata.

Run `uv run --no-project python scripts/acceptance.py` (or Python 3.11+ directly) from the repository. The harness builds actual release executables with build IDs `acceptance-a` and `acceptance-b`, packages them as ZIP assets, and serves them through an explicit loopback GitHub-compatible fixture. It uses the real provider and public CLI for every source, acquisition, claim, activation, and reclaim operation.

The target is `mock/` itself. The harness inspects and reuses an existing compatible root; it never recursively wipes it. Each run adds a distinct fixture source and keeps prior provenance. Recipes and packaged builds live in `target/acceptance/<run-uuid>/`. `target/acceptance/latest.json` records the latest successful run, build digests, UUIDs, process results, and active binding. Both `target/` and `mock/` are ignored by Git.

```text
mock/
  source/<source-uuid>.json       # release discovery; no clone directory
  index/<app-uuid>/index.json     # request-bearing claims and active binding
  index/<entry-uuid>/index.json   # output provenance or reclaimed tombstone
  store/<entry-uuid>/saucepan.exe # natural extracted contents
  bin/saucepan.exe               # stable built shim
  bin/saucepan.shim.json          # owning app UUID and Saucepan root forwarding
  .locks/                        # stable coordination locks and lifetime markers
  .staging/                      # private preparation and transaction intent
```

On Unix executable names omit `.exe`. The harness checks exact executable SHA-256 against each packaged local build. A real build-A process keeps running through activation of build B. It checks rollback, a non-executable failed candidate, non-sliding 24-hour discovery, explicit failed-check/stale behavior, independent app claims, duplicate/reuse policies, eleven-entry reclaim, and reclaimed-item reacquisition. The original output UUID remains a tombstone; reacquisition selects another live entry.

Inspect the installation on Windows:

```powershell
$root = (Resolve-Path ./mock).Path
./mock/bin/saucepan.exe --root $root --version
$report = Get-Content ./target/acceptance/latest.json -Raw | ConvertFrom-Json
./mock/bin/saucepan.exe --root $root inspect $report.app
./mock/bin/saucepan.exe --root $root path $report.active.entry_uuid
```

The fixture HTTP server exists only during the harness run. Its cached discovery can still be inspected afterward, and the installed executable runs offline. A new live acquisition after expiry needs a running fixture or a production source. Rerunning the harness creates a fresh fixture source instead of changing the production GitHub API default.

For a real release definition, register `github_release`, save its UUID in a request with explicit ZIP/raw format, platform filter, `bin`, and `useable_app` protection, then run:

```powershell
saucepan --root $root bootstrap request.json ./target/release/saucepan-shim.exe
saucepan --root $root self-update APP_UUID CLAIM_UUID
saucepan --root $root rollback APP_UUID CLAIM_UUID RETAINED_ENTRY_UUID probe.json
```

The final command validates the retained output and moves the claim and binding together; primitive `activate` remains available for binding-only changes. A reclaimed output must be reacquired first. Normal self-update never replaces the shim. `install-shim` refuses an occupied destination; use `replace-shim` for owned launcher maintenance, then `reconcile` if a running launcher leaves replacement pending. These commands return lifecycle operation records; success is `status: "complete"`.

Native process checks run with `cargo test --test managed`. On Windows use `uv run --no-project python scripts/verify_windows_console.py` for an attached but hidden console: this checks Ctrl-Break forwarding in addition to launcher death, inherited lease lifetime, arguments, streams, exit status, failed activation, and rollback.

Windows x86_64 execution is verified. Linux and macOS code paths have not been executed in this environment. The native lease contract requires targets to preserve the inherited lifetime handle. This implementation has no legacy encrypted-index/token/HMAC compatibility, migration decoder, schema-version inheritance, installation scripts, or background refresh daemon. Atomic file replacement is not a guarantee of whole-filesystem power-loss durability.

## Isolated resource-access acceptance

Run `uv run --no-project python scripts/access_acceptance.py`. This builds a
fresh application, shim, and external consumer, then installs under a unique
`target/access-acceptance/<uuid>/root`. It does not change the existing mock
installation. The report records binary digests, a fresh shim launch, lightweight
lookup, explicit corruption detection, native claim access, leased consumption
after claim release and retention, and subsequent reclaim. Per-test payload IO
observations are saved alongside the report; these measure the shared native
lookup and pre-spawn boundary, not a system-wide trace of the release binary.

The latest result is `target/access-acceptance/latest.json`. Use
`uv run --no-project python scripts/verify_windows_console.py` on Windows to
exercise hidden-console interruption for both shims and leased `run` consumers.
Linux/macOS process behavior requires execution on those hosts; compilation is
not evidence of runtime parity.

When a previous Windows shim exists in `mock/bin`, the access recipe copies it
into the isolated installation to verify inherited-marker compatibility after
wrapper death. In that isolated fixture only, it starts the old reader against
the old application-record shape, then restores current metadata after the
target has acquired its lease. This tests a surviving old process across an
upgrade; it does not promise that an old launcher reads new application records.
It never replaces the original shim. See
[verification evidence](resource-access-verification.md) for scenario coverage
and the latest recorded build identities.
