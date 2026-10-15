# Language SDKs

[← Integrating an application](../README.md#integrating-an-application)

`python/` and `ts/` adapt the current Rust CLI. Rust owns policy evaluation, source retrieval, output publication and retention. SDK installation does not install the manager. No external consumer repository is migrated by this work.

The operation inventory under `contracts/` tracks argument layouts, types, identity and real-binary coverage. Profile previews, effective-policy explanations and managed installation/update/rollback/uninstall are supported. Standalone activate/deactivate and install/replace/adopt-shim commands remain CLI-only.

Paths and retention are different: bare lookup is read-only, guarded execution retains resources through the native launch boundary, and durable pins last until explicit release. Keep consumer writes outside immutable managed outputs.

## Install from a release

Saucepan 0.10.0 includes Python and TypeScript SDKs 0.2.0. Their profile, explanation and managed lifecycle methods need the matching manager contract; installing an updated SDK does not upgrade an existing executable. Obtain the manager and shim separately from the platform ZIP.

Download `saucepan_sdk-0.2.0-py3-none-any.whl` (or the Python source distribution) and `saucepan-sdk-0.2.0.tgz` from the [v0.10.0 release](https://github.com/ZackaryW/saucepan/releases/tag/v0.10.0), verify them against its `SHA256SUMS`, then install the needed package:

```sh
pip install ./saucepan_sdk-0.2.0-py3-none-any.whl
npm install ./saucepan-sdk-0.2.0.tgz
```

`sdk-manifest.json` records the package versions, hashes and associated manager tag/commit. Use that evidence when pinning consumer dependencies. These packages are distributed as release assets; no npm or PyPI publication is configured.

## Install from source

Python (3.12+):

```sh
pip install ./sdk/python
pip install -e ./sdk/python
pip install "saucepan-sdk @ git+https://github.com/ZackaryW/saucepan.git@COMMIT#subdirectory=sdk/python"
```

Replace `COMMIT` with a tested full commit. uv supports the same VCS/subdirectory input. The SDK imports as `saucepan_sdk`.

TypeScript (Node 22+):

```sh
cd sdk/ts
npm ci
npm run build
npm pack
# In a separate consumer:
npm install /absolute/path/to/sdk/ts
# Or install the built distributable:
npm install /absolute/path/to/saucepan-sdk-0.2.0.tgz
```

npm's Git dependency support does not select a package subdirectory. Use the built local folder or tarball instead. Import `Client` from `saucepan-sdk`; its client/types entry point has no eager Node filesystem/process dependency, while actual operations need the default Node transport or an explicit capable host transport.

## Context and lifetime choices

| Need | API choice |
| --- | --- |
| Public source discovery | `query_sources()` / `querySources()`, without registration or identity |
| Scoped ownership | Explicit app-bound client, then ensure with a caller-provided scope |
| Fresh original material | `retrieve` with explicit portable policy catalog and destination |
| Copied results | `acquire_retrieved` / `acquireRetrieved` |
| Read an existing selection | Claim/source/path lookup; no implicit refresh |
| Retain files for a command | Rust-backed run methods, literal argv and inherited streams |
| Delayed reads after updater exit | Explicit pin; explicitly unpin only after readers finish |

Both SDKs preserve the flat UUID policy shape. Installing an SDK does not fetch a manager, app denial does not fall back to local administration, and optional configuration remains omitted until supplied. Neither SDK supervises detached services or guarantees that arbitrary runtimes propagate inherited native handles to grandchildren. Use explicit durable pins when a host cannot preserve the documented guard lifetime.

## Profiles and managed setup

`profiles` lists recipes; `profile()` asks the manager to expand `managed`, or supply `explicit-update`/`temporary`. Preview is root-free and does not register or attach anything. Register the returned catalog explicitly and use its separate retrieval/acquisition refs, or its combined refs for a managed request. Custom catalogs and stage refs are accepted; recipes and policy precedence are implemented only in Rust. `explain_policies`/`explainPolicies` reports effective axes and UUID/digest origins without retrieving content. Explanation is observation, not authorization.

Use `for_install=True` / `{forInstall:true}` for managed setup. It requires cached storage and adds usable-app protection. `install_application`/`installApplication` takes an explicit name, request, probe, built shim, scope and shim name; `bootstrap` sets up Saucepan itself using a registered GitHub release request and matching built shim. Supply the first manager/shim yourself as described in [first setup](../docs/application-lifecycle.md#first-setup-with-a-profile). The SDK never downloads its own executable, changes PATH or chooses credentials, scope, platform or verification for you.

`update_application`/`updateApplication`, `self_update`/`selfUpdate`, `rollback`, `uninstall` and `reconcile` return `{exit_code, result}`. Exit 0 means complete; exit 2 preserves preparing, commit-intent, cleanup-pending, failed or conflict receipts. `operations` returns an ordinary list even when a receipt is noncomplete. Inspect `status`, `pending`, `reason` and `operation`; explicitly reconcile or retry the same operation UUID when appropriate. Domain errors remain `SaucepanError`. Invalid output is a protocol error and may leave a mutation indeterminate. Nothing retries or compensates automatically.

The Python and TypeScript `examples/` directories contain runnable root-free profile previews and managed setup examples. Resource consumers do not need to install a managed executable or adopt a profile. Temporary results still require a guarded run; caller-owned delivery remains explicit.

## Development

The SDKs use standard-library/Node runtime APIs. Install the Python development group and Node development dependencies for typing, tests and packaging. Dependency decisions and dated acceptance results belong to the corresponding OpenSpec change, described under [specifications and change history](../README.md#specifications-and-change-history).

Build the manager once, then run:

```sh
cargo build --locked --bins
uv run --project sdk/python --group dev python -m unittest discover -s sdk/python/tests
uv run --project sdk/python --group dev mypy sdk/python/src/saucepan_sdk sdk/python/examples
npm --prefix sdk/ts ci
npm --prefix sdk/ts test
uv run --project sdk/python python -m unittest discover -s sdk/contracts
uv run --no-project python sdk/check_packages.py build --directory target/sdk-dist
uv run --no-project python sdk/check_packages.py verify --directory target/sdk-dist --source-installs
```

Set `SAUCEPAN_TEST_BINARY` to select an explicit executable for resource tests, with its matching `saucepan-shim` alongside it. Add `--binary /absolute/path/to/saucepan --shim /absolute/path/to/saucepan-shim` to installed-package verification for profile/explanation and managed lifecycle checks. No hosted run is triggered by these commands.

`contracts/operations.json` maps the current adapter methods to CLI operations. Resource fixtures use isolated roots and local Git; consumer fixtures exercise cross-language pin handover, grouped execution and scoped upgrades. They are not migrations of external applications.
