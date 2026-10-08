# Saucepan

Rust source acquisition with separate source working data, immutable outputs, and application claims. This is a new implementation; legacy stores and protocols are not migrated.

Use an explicit root. During development the final simulated installation lives in `mock/`, never the user's production `.saucepan`.

```powershell
cargo run -- --root ./scratch init
cargo run -- --root ./scratch register-app example
```

Save a source definition and register it:

```json
{"type":"local","path":"./assets"}
```

```powershell
cargo run -- --root ./scratch register-source source.json
```

The returned source UUID identifies `source/<uuid>.json`. App/output records live at `index/<uuid>/index.json`; only outputs have `store/<uuid>/` contents. Source definitions and index files are local metadata, not authentication proofs. Filesystem access follows the local OS user's trust boundary.

Save a request using the returned source UUID (replace the illustrative UUID):

```json
{"source_uuid":"00000000-0000-4000-8000-000000000001","selector":{"type":"local"},"policies":{"identical":"reuse","eviction":{"strategy":"latest","keep":10}}}
```

```powershell
cargo run -- --root ./scratch acquire request.json
cargo run -- --root ./scratch path OUTPUT_UUID
```

Acquisition returns `entry`, `directory`, `reused`, `update_checked`, `stale`, and a `reclaim` report. `new_entry` creates another output for identical inputs; `reuse` returns a compatible verified output. New content never replaces older entry bytes. Local inputs are checked on every acquisition. A saved transaction intent lets reopening recover interrupted publication and claim/binding changes.

`path` defaults to metadata checks without hashing payloads. Use `path OUTPUT_UUID --verification full` or `verify OUTPUT_UUID` for explicit integrity verification. Bare paths have no retained reclaim protection; native consumers hold `ResourceLease` guards. See [resource access](docs/resource-access.md) for ownership and verification contracts.

`models/{source,index,store,policy}` defines the contract, matching `core` modules own behavior, `cli` adapts commands, and `utils` contains domain-independent mechanics. `saucepan-shim` is a separate binary that reads its sibling `.shim.json` app identifier and the application's committed binding.

Portable helpers require caller-owned stable roots. Atomic file replacement does not imply a multi-file transaction or power-loss directory durability; native locks coordinate cooperating callers. Path validation rejects unsafe/ambiguous names; it does not isolate hostile processes running as the same user.

Build requires Rust 1.89 or newer. Dependencies are locked in Cargo.lock. All direct dependencies offer MIT or Apache-2.0 licensing; Rust standard native file locks avoid an extra lock dependency. ZIP enables Deflate only; HTTP uses rustls and JSON support.

Repository sources accept `{"type":"git","origin":"C:/repos/project"}` or `{"type":"github_repo","repo":"owner/project"}`. Registering equivalent normalized origins returns the existing source UUID. Use `{"type":"ref","reference":"main"}`, `{"type":"tag","tag":"v1"}`, or `{"type":"commit","commit":"FULL_OBJECT_ID"}` as the selector. Natural format exports the exact commit, optionally selecting `folder`, from reusable `source/<uuid>/repo` data. Subsequent fetches leave old store entries unchanged. Git and Git LFS must be installed when required by the source; dependency failures fail acquisition. Committed links are materialized as ordinary files within the tree before folder selection.

The process utility uses `process-wrap` (MIT/Apache-2.0) with Windows Job Objects or Unix process groups, concurrent bounded pipe capture, and deadlines. Descendants must remain within their assigned job/group. Git export additionally bounds tree entries/bytes and records exact submodule commits and LFS digests. `cargo test --test git` runs the two-revision CLI/native example; `cargo test --test remote` exercises release discovery, expiry, failed checks, recovery, ZIP integrity, and platform selection against loopback HTTP.

Run `cargo test --all-targets` for the currently implemented contracts and portable filesystem/process fixtures.

For release assets, register `{"type":"github_release","repo":"owner/project"}` and use a request with `selector: {"type":"latest_stable"}` (or `{"type":"tag","tag":"v1.0"}`). A raw executable selection is:

```json
{"format":"raw","filters":[{"platform":{"os":"windows","arch":"x86_64"},"asset_patterns":["*-windows-x64.exe"]}],"bin":["tool.exe"],"filename":"tool.exe"}
```

Put that object in the request's `selection`. For ZIP assets use `format: "zip"` with optional `folder` and binary-relative paths. Multiple matches in the winning filter are errors. URL sources (`{"type":"url","url":"https://example.com/tool.zip"}`) use selector `{"type":"url"}`, explicit format, and `filename`.

Remote discovery is cached for 24 hours from a successful check. `--force-refresh` bypasses the interval; `--allow-stale` explicitly permits old discovery after failed checks and reports `stale: true`. Reads never extend freshness. Local inputs have no discovery TTL. Release sources do not clone Git repositories. The hidden `--test-github-api http://127.0.0.1:PORT` and `--test-now SECONDS` options support deterministic isolated fixtures without changing production defaults.

To associate a request with an application, use `acquire request.json --app APP_UUID`. The result includes a claim UUID and the acquired output. `update-claim APP_UUID CLAIM_UUID` reacquires that claim's original request; `release-claim APP_UUID CLAIM_UUID` only removes that association. `claims APP_UUID` reports that app's claims and availability. `settings APP_UUID settings.json` accepts `{"source_filter":["SOURCE_UUID"]}` to filter this view; filters do not create claims or grant access. Two apps can share one output, then update independently.

Every acquisition evaluates latest-ten retention for its source/selector/selection item. `policies.eviction` can override the positive count. Coexisting attached counts use the maximum; claims preserve their policy demand but ordinary claims do not prevent reclaim. Reuse preserves creation order. `reclaim` reports effective counts, deferred protected/in-use entries, and failed deletions as pending. Provenance remains after content deletion. `path` refuses unavailable entries; `reacquire ENTRY_UUID` gets a live output, while `update-claim` also moves the application's claim.

Managed executables declare `selection.bin`, a matching `selection.platform`, and `policies.protection: {"type":"useable_app"}`. Save a probe such as `{"args":["--version"],"stdout_contains":"saucepan ","timeout_seconds":10}`. Run `activate APP_UUID ENTRY_UUID saucepan.exe probe.json`, then `install-shim APP_UUID saucepan.exe PATH_TO_BUILT_SHIM --saucepan`. Activation validates and probes before atomically switching the binding. Downloading alone never switches it. Activating a retained older entry performs rollback. Routine updates leave the stable shim untouched; launcher replacement is a separate maintenance operation.

Active useable_app bindings and native execution leases can exceed the retention count. The dedicated shim inherits a lifetime handle into its target: Windows checks sharing conflicts, Unix checks the inherited file lock. Launcher death does not make a still-running target reclaimable. Targets must preserve the inherited lifetime handle; intentionally closing inherited handles or daemonizing outside this lifetime contract is unsupported. Windows console interruption reaches the target's console group; Unix replaces the shim with the target, preserving normal signals. Supported execution evidence is recorded separately for each OS.

Saucepan's own release request can be installed with `bootstrap request.json PATH_TO_BUILT_SHIM`; the response includes app and claim UUIDs. `self-update APP_UUID CLAIM_UUID` acquires and probes a replacement through the release provider. All commands require `--root`; Saucepan's shim propagates its root and rejects a conflicting one.

