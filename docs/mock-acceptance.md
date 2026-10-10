# Simulated installation

This is historical checkpoint evidence. Its earlier record layout and scripts do
not define current schema compatibility. Use [claim registry verification](claim-verification.md)
for the current implementation and runnable acceptance commands.

For current authenticated-store acceptance, use `cargo test --offline --test claim_acceptance`. It uses real built binaries and local source inputs in a disposable root while preserving `mock/`. It is not a rerun of the historical two-build release recipe. See the [documentation map](README.md) for that evidence gap. The historical `scripts/lifecycle_acceptance.py` run is recorded in [lifecycle verification](lifecycle-verification.md); its earlier-layout harness is not a current setup recipe.

The original run used `uv run --no-project python scripts/acceptance.py` from the repository. That harness built actual release executables with build IDs `acceptance-a` and `acceptance-b`, packaged them as ZIP assets, and served them through an explicit loopback GitHub-compatible fixture. It used the provider and CLI of that checkpoint for every source, acquisition, claim, activation, and reclaim operation. Do not rerun this old-layout recipe against the preserved mock or a current store.

The historical target was `mock/` itself. The harness inspected and reused a root compatible with that build; it did not recursively wipe it. Each run added a distinct fixture source and kept prior provenance. Recipes and packaged builds lived in `target/acceptance/<run-uuid>/`. `target/acceptance/latest.json` recorded the latest successful run at that checkpoint. These ignored local reports may be absent from a fresh checkout. Both `target/` and `mock/` are ignored by Git. The layout below is historical; current claims live separately under `claims/<app-uuid>/`.

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

The historical installation was inspected on Windows using its own retained binary:

```powershell
$root = (Resolve-Path ./mock).Path
./mock/bin/saucepan.exe --root $root --version
$report = Get-Content ./target/acceptance/latest.json -Raw | ConvertFrom-Json
./mock/bin/saucepan.exe --root $root inspect $report.app
./mock/bin/saucepan.exe --root $root path $report.active.entry_uuid
```

The fixture HTTP server exists only during the harness run. Its cached discovery can still be inspected afterward, and the installed executable runs offline. A new live acquisition after expiry needs a running fixture or a production source. Rerunning the harness creates a fresh fixture source instead of changing the production GitHub API default.

For current real-release usage, choose a freshly initialized root as `$root` and follow [application lifecycle](application-lifecycle.md). Register `github_release` and save its UUID in a request with ZIP/raw format, platform selection, `bin`, and `useable_app` protection. The commands are:

```powershell
saucepan --root $root bootstrap request.json ./target/release/saucepan-shim.exe
saucepan --root $root self-update APP_UUID CLAIM_UUID
saucepan --root $root rollback APP_UUID CLAIM_UUID RETAINED_ENTRY_UUID probe.json
```

The final command validates the retained output and moves the claim and binding together; primitive `activate` remains available for binding-only changes. A reclaimed output must be reacquired first. Normal self-update never replaces the shim. `install-shim` refuses an occupied destination; use `replace-shim` for owned launcher maintenance, then `reconcile` if a running launcher leaves replacement pending. These commands return lifecycle operation records; success is `status: "complete"`.

Native process checks run with `cargo test --test managed`. On Windows use `uv run --no-project python scripts/verify_windows_console.py` for an attached but hidden console: this checks Ctrl-Break forwarding in addition to launcher death, inherited lease lifetime, arguments, streams, exit status, failed activation, and rollback.

Windows x86_64 execution is verified. Linux and macOS code paths have not been executed in this environment. The native lease contract requires targets to preserve the inherited lifetime handle. This implementation has no legacy encrypted-index/token/HMAC compatibility, migration decoder, schema-version inheritance, installation scripts, or background refresh daemon. Atomic file replacement is not a guarantee of whole-filesystem power-loss durability.

## Isolated resource-access acceptance

The historical run used `uv run --no-project python scripts/access_acceptance.py`.
It built a fresh application, shim, and external consumer, then installed under a
unique `target/access-acceptance/<uuid>/root`. It did not change the existing mock
installation. The report records binary digests, a fresh shim launch, lightweight
lookup, explicit corruption detection, native claim access, leased consumption
after claim release and retention, and subsequent reclaim. Per-test payload IO
observations are saved alongside the report; these measure the shared native
lookup and pre-spawn boundary, not a system-wide trace of the release binary.

That checkpoint's latest result was `target/access-acceptance/latest.json`.
The recorded run also used `scripts/verify_windows_console.py` on Windows to
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
