---
name: develop-saucepan
description: Integrate, use, or extend Saucepan as a composable artifact source store. Use when adding Saucepan to another repository, vendoring an SDK through a pinned sparse submodule, invoking the CLI, or developing and testing the Rust CLI and Python, TypeScript, or shell SDKs.
---

# Develop with Saucepan

Select the independently consumable components needed by the caller. Binary acquisition logic is maintained outside this repository.

| Component | Sparse path | Purpose |
|---|---|---|
| Rust CLI | `src/` plus root `Cargo.toml` and `Cargo.lock` | Install, update, query, and locate artifacts |
| TypeScript SDK | `sdk/typescript/` | Typed Node.js client for the current central-store CLI |
| Shell SDK | `sdk/shell/` | POSIX functions forwarding the current CLI's JSON and exit status |
| Python SDK | `sdk/python/` | Synchronous Python client for the current central-store CLI |

Read [references/component-contracts.md](references/component-contracts.md) before changing a public contract or implementing an integration. The SDK uses a supplied CLI and does not acquire binaries.

## Choose the smallest component set

1. Inspect the consumer repository, its `AGENTS.md`, package manager, existing `.gitmodules`, and vendor conventions.
2. Identify whether the caller needs the CLI, SDK, or both.
3. Reuse an already installed CLI when appropriate; do not add a source submodule merely to execute it.
4. Select an explicit Saucepan tag or commit. Never infer or download `latest`.
5. Add canonical specs only when modifying Saucepan contracts:
   - CLI development: `src tests openspec/specs`
   - SDK development: `sdk/<language> openspec/specs/<language>-sdk`

Do not create an OpenSpec change unless the user asks or the active repository's governance explicitly requires one. Existing canonical specs remain authoritative.

## Add Saucepan as a sparse submodule

Git submodules target repositories, not subdirectories. Add the whole Saucepan repository, then materialize only the selected directories inside it. Use the consumer's established vendor path; default to `vendor/saucepan` when none exists.

```sh
git submodule add --name saucepan https://github.com/ZackaryW/saucepan.git vendor/saucepan
git -C vendor/saucepan fetch --tags
git -C vendor/saucepan checkout --detach <tag-or-commit>
git -C vendor/saucepan sparse-checkout init --cone
git -C vendor/saucepan sparse-checkout set <required-path> [<required-path> ...]
```

Examples:

```sh
# TypeScript SDK with a separately supplied binary
git -C vendor/saucepan sparse-checkout set sdk/typescript

# Rust CLI development; cone mode keeps root Cargo files visible
git -C vendor/saucepan sparse-checkout set src tests openspec/specs
```

The parent repository records the submodule commit but not its local sparse-checkout settings. Add or update the consumer's bootstrap instructions so every fresh clone runs:

```sh
git submodule update --init --recursive
git -C vendor/saucepan sparse-checkout init --cone
git -C vendor/saucepan sparse-checkout set <required-path> [<required-path> ...]
```

When the submodule already exists, inspect its pinned commit and sparse paths before changing either:

```sh
git submodule status -- vendor/saucepan
git -C vendor/saucepan sparse-checkout list
git -C vendor/saucepan status --short --branch
```

Preserve existing paths when expanding the sparse set; `sparse-checkout set` replaces the prior selection.

## Acquire and connect the binary

Use the consumer's externally maintained binary acquisition logic, or build the selected Saucepan revision. This repository no longer ships a binary downloader. If the submodule points to an unreleased commit, build that commit instead of pairing it with an unrelated release:

```sh
cargo build --release --manifest-path vendor/saucepan/Cargo.toml
```

Pass the resulting path explicitly to integrations when reproducibility matters.

## Use the CLI or SDK

Register the app with the central store. Its settings and initial filters are saved in the encrypted index; no local TOML settings are read.

```sh
<saucepan-binary> init
<saucepan-binary> register example-app > .saucepanhash
<saucepan-binary> --marker .saucepanhash acquire recipe.json
<saucepan-binary> --marker .saucepanhash view
```

Build the TypeScript package before consuming it from a local path:

```sh
cd vendor/saucepan/sdk/typescript
npm ci --ignore-scripts
npm run build
```

```typescript
import { Saucepan } from 'saucepan-sdk';
const app = new Saucepan({ binary: '.tools/saucepan', marker: '.saucepanhash' });
const acquired = await app.acquire({ source: { provider: 'local', path: './assets' } });
console.log(acquired.directory);
```

Alternatively source `sdk/shell/saucepan.sh` and call `saucepan_acquire recipe.json`. Follow each SDK's README for installation and options. Use returned paths rather than reconstructing storage layout. Keep recipes, caching, authority, and view filtering in the core. Do not introduce a binary downloader, TOML configuration, or legacy exit-code mapping into the new adapters.

## Develop through the submodule

Treat the consumer and `vendor/saucepan` as separate Git repositories.

1. Confirm the requested change belongs upstream rather than in consumer glue.
2. Enter the submodule and create or select an upstream branch before committing; submodules normally start detached.
3. Make the smallest component-scoped change and preserve the contracts in the reference.
4. Run the component's tests from the Saucepan submodule root.
5. Commit or hand off the upstream change according to user authorization and repository policy.
6. Return to the consumer, verify the integration, and record the updated submodule gitlink separately.

Do not stage submodule source files from the parent repository. Do not silently advance to a moving branch, rewrite published submodule history, or push either repository without authorization.

## Validate

Run checks in proportion to the selected components:

```sh
# Rust CLI
cargo test --manifest-path vendor/saucepan/Cargo.toml

# Build the real executable before adapter integration tests
cargo build --locked --manifest-path vendor/saucepan/Cargo.toml

# TypeScript SDK (from its package directory)
cd vendor/saucepan/sdk/typescript
npm ci --ignore-scripts
npm test
npm pack --dry-run

# Shell SDK (from the Saucepan repository root)
node --test sdk/shell/tests/run.mjs
```

The TypeScript runtime uses only Node.js built-ins; the POSIX library requires only the shell and executable. Both integration suites use explicit test roots and keys. Set `SAUCEPAN_TEST_BINARY` to test another build and `SAUCEPAN_TEST_SHELL` for a specific POSIX shell (for example Git Bash on Windows). Run the Python client suite with `uv run --project sdk/python pytest sdk/python/tests` and its behavior scenarios with `uv run --project sdk/python behave sdk/python/features/python-sdk`. For consumer work, also run the consumer's relevant tests against the pinned binary and submodule commit.

## Hand off

Report:

- selected component paths and why each is needed;
- submodule path and pinned Saucepan commit/tag;
- binary acquisition method and version;
- consumer integration files changed;
- tests run and their results;
- whether fresh clones need a sparse-checkout bootstrap step.
