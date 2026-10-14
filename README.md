# Saucepan

Rust source acquisition with separate source working data, immutable outputs, and application claims. This is a new implementation; legacy stores and protocols are not migrated.

See the [documentation map](docs/README.md) for current usage, specification ownership, and dated verification evidence.

[Composable retrieval and acquisition](docs/composition.md) separates original source material from copied results. `retrieve` and `source-path` expose source material; `acquire-retrieved` copies or extracts it. `run-retrieved` and `run-acquired` retain paths for child processes. Inline temporary/no-store requests need no home, store, app or keyring. Cached retrieval, acquisition updates and revision history have independent policies.

[Release workflows](docs/releases.md) build platform ZIPs containing both the manager and shim, with checksums and native CI checks before publication.

Managed commands default to `%USERPROFILE%\.saucepan` on Windows or `~/.saucepan` on Unix. Initialize it once, then query publicly discoverable sources without an app UUID or query file:

```powershell
cargo run -- init
cargo run -- query-sources
cargo run -- query-sources --type github_release --repo owner/project
```

New stores use basic checksum integrity and need no OS keyring setup. To opt into
HMAC metadata protection, initialize a separate root with
`saucepan --root ./protected init --integrity keyring`. Later commands and shims
use the stored mode automatically. An unavailable protected key fails access;
it never switches the store to basic mode. App credentials remain separately
opt-in; credential-protected apps require a keyring-mode store.

Sources are private until explicitly published, so an empty anonymous query can be expected. Use `--root ./scratch` for a separate store; the override always wins and never falls back to the default. Queries do not initialize missing stores. The historical `mock/` installation remains an earlier acceptance fixture and is not migrated. The following acquisition examples use a fresh disposable root:

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

The returned source UUID identifies `source/<uuid>.json`. App/output records live at `index/<uuid>/index.json`; only outputs have `store/<uuid>/` contents. Claims live in `claims/<app-uuid>/index.json` and `<claim-uuid>.json`. `root.json` records verified catalog membership. Source, index, claim and lifecycle records use readable checksummed JSON in basic mode, or HMAC-SHA256 with a native OS keyring key in protected mode. Earlier unsigned layouts remain unsupported. Basic checksums detect inconsistency but cannot detect a writer who recomputes all checksums; filesystem access follows the local OS user's trust boundary. See [integrity and custody](docs/claim-integrity.md).

Find registered sources before claiming with `query-sources`, optionally adding `--type github_release --repo owner/project`. A saved `query-sources query.json` remains supported instead of flags. Use `--as-app APP_UUID` for that application's permitted sources; without it, only explicitly published sources appear. See [source discovery and the simple app flow](docs/source-query.md).

Save a request using the returned source UUID (replace the illustrative UUID). Empty references use built-in behavior; custom behavior is registered as immutable records with `register-policies`, then attached by UUID. See the [policy guide](docs/policy-resources.md). Old `policies`, `eviction`, inline filters and caller-supplied execution evidence are rejected:

```json
{"source_uuid":"00000000-0000-4000-8000-000000000001","selector":{"type":"local"},"policy_refs":[]}
```

```powershell
cargo run -- --root ./scratch acquire request.json
cargo run -- --root ./scratch path OUTPUT_UUID
```

Acquisition returns `entry`, `directory`, `reused`, `update_checked`, `stale`, and a `reclaim` report. `new_entry` creates another output for identical inputs; `reuse` returns a compatible verified output. New content never replaces older entry bytes. Local inputs are checked when preparing outputs; ensuring an available compatible claim reuses its selection without refreshing the source. A saved transaction intent lets reopening recover interrupted publication and claim/binding changes.

`path` defaults to metadata checks without hashing payloads. Use `path OUTPUT_UUID --verification full` or `verify OUTPUT_UUID` for explicit integrity verification. Bare paths have no retained reclaim protection; native consumers hold `ResourceLease` guards. See [resource access](docs/resource-access.md) for ownership and verification contracts.

Use `run-group group.json -- PROGRAM` to retain several named prepared outputs for
one child, or `Store::lease_group` / `AppSession::lease_group` for native callers.
App identity and group policy boundaries are optional; claim targets require an
app context. Groups do not fetch or update resources. See [group leases](docs/group-leases.md)
for payloads, flat policy references and the runnable consumer examples.

The [Power Eagle and Zush integration assessment](docs/consumer-integration-assessment.md)
separates current capabilities, proposed group orchestration and consumer-owned
catalog, activation and lifetime work.

`models/{source,retrieval,index,store,policy,claim}` defines the contract, matching `core` modules own behavior, `cli` adapts commands, and `utils` contains domain-independent mechanics. `saucepan-shim` is a separate binary that reads its sibling `.shim.json` app identifier and the application's committed binding.

Portable helpers require caller-owned stable roots. Atomic file replacement does not imply a multi-file transaction or power-loss directory durability; native locks coordinate cooperating callers. Path validation rejects unsafe/ambiguous names; it does not isolate hostile processes running as the same user.

Build requires Rust 1.89 or newer. Dependencies are locked in Cargo.lock. All direct dependencies offer MIT or Apache-2.0 licensing; Rust standard native file locks avoid an extra lock dependency. ZIP enables Deflate only; HTTP uses rustls and JSON support.

Repository sources accept `{"type":"git","origin":"C:/repos/project"}` or `{"type":"github_repo","repo":"owner/project"}`. Registering equivalent normalized origins returns the existing source UUID. Use `{"type":"ref","reference":"main"}`, `{"type":"tag","tag":"v1"}`, or `{"type":"commit","commit":"FULL_OBJECT_ID"}` as the selector. Natural format exports the exact commit, optionally selecting `folder`, from reusable `source/<uuid>/repo` data. Subsequent fetches leave old store entries unchanged. Git and Git LFS must be installed when required by the source; dependency failures fail acquisition. Committed links are materialized as ordinary files within the tree before folder selection.

The process utility uses `process-wrap` (MIT/Apache-2.0) with Windows Job Objects or Unix process groups, concurrent bounded pipe capture, and deadlines. Descendants must remain within their assigned job/group. Git export additionally bounds tree entries/bytes and records exact submodule commits and LFS digests. `cargo test --test git` runs the two-revision CLI/native example; `cargo test --test remote` exercises release discovery, expiry, failed checks, recovery, ZIP integrity, and platform selection against loopback HTTP.

Run `cargo test --all-targets` for the currently implemented contracts and portable filesystem/process fixtures.

For release assets, register `{"type":"github_release","repo":"owner/project"}` and use a request with `selector: {"type":"latest_stable"}` (or `{"type":"tag","tag":"v1.0"}`). A raw executable selection is:

```json
{"format":"raw","platform":{"os":"windows","arch":"x86_64"},"filter_refs":["00000000-0000-4000-8000-000000000002"],"bin":["tool.exe"],"filename":"tool.exe"}
```

Put that object in the request's `selection`. For ZIP assets use `format: "zip"` with optional `folder` and binary-relative paths. Multiple matches in the winning filter are errors. URL sources (`{"type":"url","url":"https://example.com/tool.zip"}`) use selector `{"type":"url"}`, explicit format, and `filename`.

Remote discovery is cached for 24 hours from a successful check. A referenced `refresh_policy` with `mode: "always"` bypasses the interval; `stale_policy` with `allow_stale: true` permits cached material after failed checks and reports `stale: true`. Reads never extend freshness. Local inputs have no discovery TTL. Release sources do not clone Git repositories. The hidden `--test-github-api http://127.0.0.1:PORT` and `--test-now SECONDS` options support deterministic isolated fixtures without changing production defaults.

To associate a request with an application, use `acquire request.json --app APP_UUID --scope PROJECT_KEY`. The result includes a claim UUID and the acquired output. `update-claim APP_UUID CLAIM_UUID` reacquires that claim's original request; `release-claim APP_UUID CLAIM_UUID` only removes that association. `claims APP_UUID` reports that app's claims and availability. `settings APP_UUID settings.json` accepts `{"source_filter":["SOURCE_UUID"]}` to filter this view; filters do not create claims or grant access. Two apps can share one output, then update independently.

Attach project or alias labels with `acquire request.json --app APP_UUID --scope PROJECT_KEY --context context.json`, where the context file is a plain string map. Search with `query-claims query.json` using `{ "app": "APP_UUID", "where": [{ "key": "project", "op": "eq", "value": "my-project" }] }`. Operators are `eq` and explicit string `glob`; all conditions must match. Results include the application revision and every matching claim in UUID order. `set-claim-context APP_UUID CLAIM_UUID context.json --expected-revision N` replaces labels without moving the claim. See [claim context and queries](docs/claim-query.md) for complete examples, wildcard syntax, and lease boundaries. Run `cargo test --test query_cli` for the isolated two-project CLI flow.

Every managed convenience acquisition evaluates latest-ten retention for its source/selector/selection item. Attach a registered `revision_policy` UUID through `policy_refs` to select history retention. Coexisting attached counts use the maximum; claims preserve their policy demand but ordinary claims do not prevent reclaim. Reuse preserves creation order. `reclaim` reports effective counts, deferred protected/in-use entries, and failed deletions as pending. Provenance remains after content deletion. `path` refuses unavailable entries; `reacquire ENTRY_UUID` gets a live output, while `update-claim` also moves the application's claim.

Register the release filter above as a `github_release_asset_filter` record before using its UUID.

Managed executables declare `selection.bin`, a matching `selection.platform`, and a registered `useable_app_policy` UUID in `policy_refs`. First ensure or acquire the executable as a claim owned by the app in an allowed scope. Save a probe such as `{"args":["--version"],"stdout_contains":"saucepan ","timeout_seconds":10}`. Run `activate APP_UUID ENTRY_UUID saucepan.exe probe.json`, then `install-shim APP_UUID saucepan.exe PATH_TO_BUILT_SHIM --saucepan`. Activation validates and probes before atomically switching the binding. Downloading alone never switches it. Activating a retained older entry accessible through an authorized claim performs binding-only rollback; use managed `rollback` for the coordinated claim/binding transition below. Routine updates leave the stable shim untouched; launcher replacement is a separate maintenance operation.

Active useable_app bindings and native execution leases can exceed the retention count. The dedicated shim inherits a lifetime handle into its target: Windows checks sharing conflicts, Unix checks the inherited file lock. Launcher death does not make a still-running target reclaimable. Targets must preserve the inherited lifetime handle; intentionally closing inherited handles or daemonizing outside this lifetime contract is unsupported. Windows console interruption reaches the target's console group; Unix replaces the shim with the target, preserving normal signals. Supported execution evidence is recorded separately for each OS.

Use `deactivate APP_UUID` to clear an application's active binding. Claims, cached outputs, installed shims and running processes remain intact. New shim launches fail until reactivation; subsequent `reclaim` follows retention and remaining live/active protection. Repeating deactivation succeeds as a no-op. See [resource access](docs/resource-access.md) for concurrency and result details.

For a coordinated application install, use `install NAME request.json probe.json PATH_TO_BUILT_SHIM --shim NAME.exe --scope APPLICATION_KEY`. `update-app APP_UUID CLAIM_UUID probe.json` and `rollback APP_UUID CLAIM_UUID ENTRY_UUID probe.json` validate the candidate before committing the claim and binding together. Failed validation preserves both selections; an intervening app mutation reports a conflict. The primitive `activate` and `update-claim` commands retain their separate meanings.

Saucepan's own release request can be installed with `bootstrap request.json PATH_TO_BUILT_SHIM`; `self-update APP_UUID CLAIM_UUID` uses the same coordinated update flow. Multi-step commands return an operation record with `operation`, `app`, `claim`, `candidate`, `status` and pending reasons. Retrying with the same `--operation UUID` and inputs resumes work or returns its recorded result. Pending, failed and conflicted results exit 2. Direct CLI commands default to the user-home `.saucepan` store; `--root` overrides it. Saucepan's shim propagates its installed root and rejects a conflicting one.

`replace-shim` maintains a registered launcher, while `adopt-shim` explicitly registers a verified existing pair. `uninstall APP_UUID` releases claims and disables new launches before cleaning verified owned files; cached outputs and existing consumers remain subject to normal retention and leases. Busy or changed files leave inspectable pending work. Use administrative `operations` and `reconcile --operation OPERATION_UUID` from a directly invoked binary to inspect receipts and finish cleanup after the launcher exits. `operations --app APP_UUID` is available while the app registration remains usable and applies its policies and optional credentials.

See [application lifecycle](docs/application-lifecycle.md) for commands, ownership and recovery rules, and [current verification evidence](docs/policy-verification.md) for executed scenarios. Earlier script reports describe historical checkpoints. Earlier layouts are rejected without migration. Current claim-registry acceptance is covered by `cargo test --offline --test claim_acceptance`; it uses the built binary and shim in a disposable root.


The Rust app-facing API is `Store::session(app, optional_credential)`. It evaluates
source, scope and action restrictions independently of whether proof is enabled.
Use `register_application` with an explicit registration key and
`configure_application` with an observed revision for administrative changes.
Compatible same-scope requests share one claim; contained natural folders return
a relative view of that owner. Different scopes update independently. Recovery
uses `report_unavailable` (default), `restore_exact`, or `refresh`.
See [application policy contracts](docs/claim-policies.md), [CLI commands and JSON
errors](docs/claim-cli.md), the [native consumer example](docs/scoped-consumer.md),
and [current verification evidence](docs/policy-verification.md).
