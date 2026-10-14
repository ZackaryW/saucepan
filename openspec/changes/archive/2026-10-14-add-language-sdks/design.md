# Design

## Context

See proposal.md for motivation. There is no SDK directory in this checkout. `src/cli/mod.rs` already accepts optional root/app/credential context and JSON errors; composition commands accept JSON files and portable leaf-policy catalogs. `registered_groups.rs` and the pin commands expose the recently implemented group/pin contract. Temporary material requires a retained child or caller-owned destination. A JSON descriptor cannot carry the Rust guard across process exit.

`scripts/release.py` currently validates an exact seven-ZIP binary asset set. `release.yml` publishes those ZIPs and checksums; `core-platforms.yml` runs only manually or through release and supports selected platforms. SDK packages must be explicitly incorporated into this allowlist rather than weakening validation or running an additional complete Rust matrix per language.

Observed consumers: PowerSpec and Zush require Python 3.12+, and their adapters use the former Python SDK. Power Eagle is a Vite/ESM application whose host accesses subprocesses through a runtime Node bridge. New SDKs target this implementation only; external adapters and binary provisioning remain consumer work.

The implemented but unarchived `orchestrate-resource-groups` change supplies group/pin requirements absent from synchronized main specs. This change depends on that implementation, leaves its artifacts intact, and does not invent replacement requirements for those features.

## Goals / Non-Goals

**Goals:** language-native calls with current typed wire contracts; inert construction; explicit identity and lifetime; predictable source/package installation; a common parity suite exercising the real Rust CLI.

**Non-goals:** PyO3/N-API/FFI, Rust policy rewrites, legacy adapters, implicit native-binary installation, browser process emulation, Power Eagle bridge implementation, external app migration, package-registry publication, managed executable/shim lifecycle wrappers, or a new daemon/IPC lease service. Initial coverage is the resource-consumer surface below, not every administrative CLI command.

## Decisions

### 1. Package ownership and structure

The repository README owns user-facing navigation, with linked guides expanding
its subsections. SDK package READMEs remain self-contained for installed artifacts.
Design and dependency decisions stay in this change; dated implementation evidence
belongs in `verification.md`. Do not create a parallel report or assessment under
`docs/` for the same change.

Use Python distribution `saucepan-sdk` with import `saucepan_sdk` and TypeScript package metadata name `saucepan-sdk`. These are local/source package names; availability of registry names is not claimed. Python supports 3.12+, synchronous calls and `py.typed`; TypeScript supports Node 22+, asynchronous calls and compiled ESM plus declarations. Do not add parallel sync/async APIs or a second JS module format without consumer evidence.

```text
sdk/
  README.md
  contracts/operations.json
  contracts/fixtures/
  python/
    pyproject.toml
    README.md
    LICENSE
    src/saucepan_sdk/
      __init__.py, client.py, errors.py, transport.py, py.typed
      models/{source,policy,claim,composition,group,pin}.py
      utils/{process,payloads}.py
    tests/
    examples/
  ts/
    package.json, package-lock.json, tsconfig.json
    README.md, LICENSE
    src/
      index.ts, client.ts, errors.ts, transport.ts
      models/{source,policy,claim,composition,group,pin}.ts
      utils/{process,payloads}.ts
    tests/
    examples/
```

Keep mechanics in each language's `utils`, command/JSON interpretation in `transport`, and operation grouping/context in `client`. Avoid a giant dynamic command dispatcher as the public API. Models describe Rust wire records using Python TypedDict/Literal and TS interfaces/discriminated unions. Small response checks establish required shapes/discriminants; they do not re-evaluate policy or perform store validation. Unsupported or malformed responses fail explicitly; no earlier-schema coercion.

Client/transport interfaces should permit injection for tests and an eventual host bridge. The TS transport-independent client/types export must not eagerly import Node modules; the default Node transport owns filesystem/process imports. A custom transport is explicit caller code, not implicit detection of `window.require`. A browser-only runtime without a capable transport cannot run Saucepan.

### 2. Explicit context and operation inventory

Client options: executable path/name (default `saucepan`), optional root, optional operation timeout, and optional transport. Construction/import never spawns or writes. An app-bound client is an immutable explicit derived context with app UUID and optional caller-owned credential-file path. No default scope, anonymous app registration, policy materialization or keyring access. Omitted root stays omitted so Rust resolves its default; root-free composition remains root-free. No automatic version probe or claim of arbitrary binary compatibility; the release manifest records the tested binary build.

Both languages expose equivalent operations, using snake_case in Python and camelCase in TS:

| Family | Initial CLI mappings |
| --- | --- |
| Setup and records | `init` with explicit integrity selection, `register-application`, `configure-application`, `register-source`, `inspect` |
| Discovery | `query-sources` with no query required, `query-claims`, `discover-claims`, `claims`, `set-source-visibility` |
| Policies | `register-policies`, `policy`, `adopt-app-policies`, `adopt-source-policies`, `adopt-claim-policies` |
| Claims | `acquire` with explicit app/scope when claimed, `ensure-claim`, `update-claim`, `recover-claim`, `release-claim`, `set-claim-context` |
| Composition and lookup | `retrieve`, `acquire-retrieved`, `source-path`, `path`, `verify`, `reclaim`, `purge-retrieval` |
| Groups | `register-group`, `show-group`, `list-groups`, `replace-group`, `remove-group`, `prepare-group` with optional explicit pin request |
| Pins | `pin-outputs`, `pin-group`, `pin`, `pins`, `unpin` |
| Guarded execution | `run`, `run-claim`, `run-group`, `run-registered-group`, `prepare-run-group`, `run-retrieved`, `run-acquired` |

`contracts/operations.json` will record corresponding SDK names, CLI argument layout, request/response types, JSON-versus-run mode, owner requirements and at least one real-binary test reference. It is a coverage inventory, not another policy/schema engine. Do not expose hidden Rust test switches as normal client options. Tests may run a controlled CLI wrapper or use native existing fixtures for test clock/loopback setup.

Operation payloads retain the current flat `policy_refs` shape. Portable catalogs are explicit parameters only on CLI-supported composition/run operations; persistent references must already be registered. Omit absent optional fields; do not replace omission with null or fill defaults in Python/TS. Arguments carrying app IDs or app-bearing query payloads must agree with bound context before spawning. Trusted administration remains explicit, and a denied app operation never retries without identity.

### 3. Transport and failures

Ordinary operations use argv arrays, shell disabled, UTF-8 JSON and `--error-format json`. A per-call temporary directory holds closed JSON input files, including portable catalogs where needed; Windows can reopen the files. Requests are data, never concatenated shell commands. Credential files are caller-owned paths, never inlined into argv, logs, payload dumps or SDK temporary files. Preserve the CLI's 1 MiB group/pin input limit; transport capture has a configurable finite byte cap (16 MiB per stream by default) and JSON calls have a configurable 120-second default timeout.

Transport errors distinguish missing executable, spawn failure, timeout, output limit and malformed/unexpected protocol from `SaucepanError(code, message, details)`. Domain failures are parsed from the documented stderr envelope, never English substrings or exit-code guesses. Exceptions must not echo request contents or credential data. Preserve successful response objects and returned revisions/provenance; typed wire declarations plus parity fixtures catch drift without policy duplication.

A timeout or broken response after a mutating command can have an indeterminate outcome. Report that possibility; never retry, unpin, remove groups, or generate replacement keys automatically. Callers reconcile using lookup and the original request key/revision. Operation metadata identifies possible mutation for error reporting only, not guessed rollback.

### 4. Lifetime is an operation choice

Bare lookup/preparation returns records and addresses; it does not pretend to return a live in-process lease. Pin creation/preparation is explicit, and unpin is a separate explicit call. No destructor, context-manager exit, failed response or SDK process exit releases a durable pin. A pin retry can return an existing receipt; automatically releasing it would incorrectly take ownership of someone else's existing retention obligation.

Run methods invoke the existing Rust launch commands directly with `--` and literal child argv. Inherit streams by default; optional bounded capture is a separate execution option. Return `RunResult` with observed exit status or signal and captured streams if requested. Never parse child stdout as an SDK response or classify a child's JSON-looking stderr as a Saucepan domain error. The current CLI cannot unambiguously distinguish a preflight failure from arbitrary child diagnostics on this channel; preserve its raw outcome and document that boundary. Spawn/transport failures before a process exists remain SDK transport errors.

Run operations have no default execution timeout. Explicit cancellation/deadline must reap the process the SDK owns, preserve observed status and avoid asserting that all descendants exited. Rust's documented inherited-handle contract remains responsible for surviving consumers; do not release pins or delete native descriptors/markers. SDK-owned request scratch is independent of Rust's lifetime-owned output/descriptor storage. Test interruption separately on each supported OS; do not claim unsupported descendant handle propagation.

### 5. Reuse and portable utility plan

This is the `pspec-plan-utilities` portion; no helpers are implemented during planning.

| Utility | Portable contract and effects | Reuse and focused verification |
| --- | --- | --- |
| Process invocation | `invoke(executable, argv, cwd, env, timeout, capture_limit, stdio)` returns exit/signal and optional byte streams; owns only its child and capture buffers. Useful for any compiler or Git adapter. Must drain both pipes, bound capture, handle cancellation/timeout, and never use shell expansion. | Python `subprocess.Popen` with standard threading for bounded pipe drains; Node `child_process.spawn` with stream events. Test real neutral child processes for spaces/Unicode/metacharacters, simultaneous streams, malformed UTF-8, large output, missing executables, deadlines and cleanup. Do not write custom shell quoting or process-supervision framework. |
| JSON payload workspace | `payload_files(values)` provides paths in one per-call temporary directory, serializes explicit JSON-compatible values and removes only owned files after use. Useful for any CLI that accepts JSON files. Caller supplies values and applicable size bound. | Python `tempfile.TemporaryDirectory`, `json`; Node `fs.mkdtemp`, explicit writes/close, `JSON.stringify`. Verify Unicode, empty/omitted values, non-JSON inputs, limits, concurrent calls and cleanup on error with real filesystem fixtures. Never store raw credential bytes. |

The Saucepan transport supplies flag placement, identity, wire decoders and endpoint limits. The domain client supplies typed operations. Neither belongs in generic utilities. Reuse existing Rust lifetime/transaction code through the CLI; do not port it.

Dependency decisions: no new runtime third-party dependency is required. Python's standard library covers process/files/types, and Hatchling provides wheel/sdist builds; `build` is development tooling. TypeScript uses Node built-ins, the TypeScript compiler, `@types/node`, and `node:test`; keep a lockfile. Python tests use `unittest`; add development type checking with mypy. Defer Pydantic/Zod/schema code generation because Rust remains semantic authority and the initial SDK only needs bounded envelope/shape validation. Defer Execa and custom Python async adapters because the chosen process contracts fit standard primitives; reconsider only against observed failures, not speculative convenience. No Cargo dependency is added.

Primary API references checked during planning: [Python subprocess](https://docs.python.org/3.12/library/subprocess.html), [Python tempfile](https://docs.python.org/3.12/library/tempfile.html), [Node child_process](https://nodejs.org/api/child_process.html), [Hatch builds](https://hatch.pypa.io/latest/config/build/), [npm pack](https://docs.npmjs.com/cli/v11/commands/npm-pack/). These describe library mechanics; domain decisions above come from repository contracts.

### 6. Source installation and release artifacts

Python builds must be self-contained within `sdk/python`, including README, license, typing marker and package files. Support `pip install ./sdk/python`, editable local install, and a commit-pinned Git URL ending `#subdirectory=sdk/python`; uv can use the same VCS subdirectory. Build and inspect wheel and sdist, then install from both into clean environments outside the checkout.

TypeScript source development runs `npm ci` and `npm run build` in `sdk/ts`; test installing the built local folder. `npm pack` must build via an appropriate pack hook and include compiled ESM/declarations, README and license. Installing the resulting tarball requires neither TypeScript nor Rust tooling. npm Git-root support is not Git-subdirectory support; do not document a fictitious npm `#subdirectory` command or add a root forwarding package solely to imitate it. [pip VCS documentation](https://pip.pypa.io/en/latest/topics/vcs-support/), [uv dependencies](https://docs.astral.sh/uv/concepts/dependencies/), [npm install](https://docs.npmjs.com/cli/install/).

Package versions are explicit SDK metadata, initially 0.1.0, separately maintainable from Rust. Build release artifacts from the exact release commit. Add a validated SDK manifest recording Python/TS versions, their exact wheel/sdist/tarball filenames, and tested Rust tag/commit. The release helper cross-checks it against source metadata and actual package metadata/contents, and accepts only those files plus the existing seven ZIPs and manifest. Checksums cover all payloads and the manifest. Missing, extra, duplicate, unsafe-path or inconsistent assets fail publication; never relax validation to `dist/*`. No npm/PyPI publishing credentials or jobs.

Add a reusable SDK workflow with manual selection of language and OS and a release-call mode. Keep manual/release-only triggers. Release verification covers Windows/macOS/Linux subprocess contracts with both languages, reusing each job's one locally built manager rather than duplicating Rust all-target tests or six architecture package builds. Build platform-independent SDK archives once, test installed artifacts, and make the existing publication job depend on that result. Do not dispatch hosted runs while implementing.

### 7. Verification and examples

Both suites use an explicit locally built manager, isolated roots and Git/loopback fixtures. Cover anonymous source lookup; app scopes and denied credential-free access without fallback; registered policy lookup and explicit adoption; cached and root-free composition; claim updates/context/recovery; group failure atomicity, expected-revision conflicts and prepare-and-pin retries; pin availability and explicit release; actual child streams/status and reclaim during use. Cross-language tests create in one language and inspect/use/release in the other, respecting owner identity.

Use three documented fixtures: Power Eagle-shaped pin A/read after updater exit/pin B/unpin A; Zush-shaped command/hooks/service execution including cleanup; PowerSpec-shaped lookup without refresh, initial preparation, explicit upgrade and separate consumer scopes. Do not claim real app migration. Fresh roots preserve historical `mock/`; no external repos are edited.

Transport tests use domain-neutral child helpers. Real-binary tests compare stable fields/error codes with direct CLI results, not copied implementation mocks. Package tests import/install from outside the source tree without Rust on PATH and pass the manager only when an operation is invoked. Document exact commands and tested runtime/OS versions; optional native keyring checks remain separate. Fixtures using injected custody are not native keyring evidence.

## Risks / Trade-offs

- CLI and language type drift → checked operation inventory and real-binary contract fixtures; fail malformed protocols rather than guessing an older schema.
- Subprocess launch overhead → accepted for the first SDK; no IPC service without measured consumer evidence.
- Power Eagle's webview may lack direct Node imports → isolate the default Node transport and expose a transport-independent client; actual host bridge wiring remains external work.
- JS cannot represent every Rust u64 exactly → reject integers outside the safe range on requests/responses before rounding; do not silently corrupt revisions. Python shares this documented portable contract for cross-language IDs/counts.
- Captured child stderr resembles a CLI failure → run methods preserve raw outcomes rather than inventing a child/preflight discriminator.
- Lost mutation response → expose indeterminate outcome and preserve caller key/revision; no SDK retries or compensating unpin.
- Durable pins outlive crashed callers → explicit inspection/release, with consumer lifecycle examples; no automatic expiry.
- Release job count/cost → selected manual SDK runs, one build per native test job, one package job, and no push/PR triggers.

## Migration Plan

Implement the Python package first, then TypeScript against the same inventory. Verify each coherent transport/API/package slice and create incremental zmem commits excluding active change artifacts. Add distribution and workflow changes only after local installed-package tests pass. Publish no packages or releases during apply. Update docs to identify tested builds and show the new APIs; external tools adopt in separate changes. No old SDK aliases or schema compatibility code is introduced. SDK rollback is selecting a previously tested package/binary pair, never rewriting store records or silently downgrading a binary.
