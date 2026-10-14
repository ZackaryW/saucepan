# Language SDKs

[← Integrating an application](../README.md#integrating-an-application)

`python/` and `ts/` adapt the current Rust CLI. Rust owns policy evaluation, source retrieval, output publication and retention. SDK installation does not install the manager. No external consumer repository is migrated by this work.

The operation inventory under `contracts/` tracks argument layouts, types, identity and real-binary coverage. Managed executable installation, bootstrap, activation, shim maintenance and uninstall wrappers are excluded from the initial resource-consumer API.

Paths and retention are different: bare lookup is read-only, guarded execution retains resources through the native launch boundary, and durable pins last until explicit release. Keep consumer writes outside immutable managed outputs.

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
npm install /absolute/path/to/saucepan-sdk-0.1.0.tgz
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

Both SDKs preserve the flat UUID policy shape. Installation does not fetch a manager, app denial does not fall back to local administration, and optional configuration remains omitted until supplied. Managed executable/shim lifecycle methods are outside this initial SDK surface. Neither SDK supervises detached services or guarantees that arbitrary runtimes propagate inherited native handles to grandchildren. Use explicit durable pins when a host cannot preserve the documented guard lifetime.

## Development

The SDKs use standard-library/Node runtime APIs. Install the Python development group and Node development dependencies for typing, tests and packaging. Dependency decisions and dated acceptance results belong to the corresponding OpenSpec change, described under [specifications and change history](../README.md#specifications-and-change-history).

Build the manager once, then run:

```sh
cargo build --locked --bin saucepan
uv run --project sdk/python --group dev python -m unittest discover -s sdk/python/tests
uv run --project sdk/python --group dev mypy sdk/python/src/saucepan_sdk sdk/python/examples
npm --prefix sdk/ts ci
npm --prefix sdk/ts test
uv run --project sdk/python python -m unittest discover -s sdk/contracts
uv run --no-project python sdk/check_packages.py build --directory target/sdk-dist
uv run --no-project python sdk/check_packages.py verify --directory target/sdk-dist --source-installs
```

Set `SAUCEPAN_TEST_BINARY` to select an explicit executable for resource tests. Add `--binary /absolute/path/to/saucepan` to installed-package verification to exercise an operation after installation. No hosted run is triggered by these commands.

`contracts/operations.json` maps the current adapter methods to CLI operations. Resource fixtures use isolated roots and local Git; consumer fixtures exercise cross-language pin handover, grouped execution and scoped upgrades. They are not migrations of external applications.
