# Simulated installation

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
saucepan --root $root activate APP_UUID RETAINED_ENTRY_UUID saucepan.exe probe.json
```

The final command is rollback through the same validation/probe path. A reclaimed output must be reacquired first. Normal self-update never replaces the shim. `install-shim` refuses an occupied destination; replacing the launcher is separate maintenance, performed while it is not running.

Native process checks run with `cargo test --test managed`. On Windows use `uv run --no-project python scripts/verify_windows_console.py` for an attached but hidden console: this checks Ctrl-Break forwarding in addition to launcher death, inherited lease lifetime, arguments, streams, exit status, failed activation, and rollback.

Windows x86_64 execution is verified. Linux and macOS code paths have not been executed in this environment. The native lease contract requires targets to preserve the inherited lifetime handle. This implementation has no legacy encrypted-index/token/HMAC compatibility, migration decoder, schema-version inheritance, installation scripts, or background refresh daemon. Atomic file replacement is not a guarantee of whole-filesystem power-loss durability.
