---
name: develop-saucepan
description: Integrate, use, or extend Saucepan as a composable artifact resolver. Use when an agent must add Saucepan to another repository, vendor only its Python SDK or binary resolver through a pinned git submodule and sparse checkout, acquire or invoke the CLI, create Saucepan workspaces and manifests, or develop and test the Rust CLI, resolver, or SDK without pulling unrelated components.
---

# Develop with Saucepan

Treat Saucepan as three independently consumable, composable components:

| Component | Sparse path | Purpose |
|---|---|---|
| Rust CLI | `src/` plus root `Cargo.toml` and `Cargo.lock` | Install, update, query, and locate artifacts |
| Python resolver | `resolvers/python/` | Download an explicitly versioned prebuilt CLI binary |
| Python SDK | `sdk/python/` | Drive an independently acquired CLI through an object API |

Read [references/component-contracts.md](references/component-contracts.md) before changing a public contract or implementing an integration. Keep the SDK and resolver independent: neither acquires nor imports the other.

## Choose the smallest component set

1. Inspect the consumer repository, its `AGENTS.md`, package manager, existing `.gitmodules`, and vendor conventions.
2. Identify whether the caller needs the CLI, resolver, SDK, or a union of them.
3. Reuse an already installed CLI when appropriate; do not add a source submodule merely to execute it.
4. Select an explicit Saucepan tag or commit. Never infer or download `latest`.
5. Add canonical specs only when modifying Saucepan contracts:
   - CLI development: `src tests features openspec/specs`
   - resolver development: `resolvers/python openspec/specs/python-platform-resolver`
   - SDK development: `sdk/python openspec/specs/python-sdk`

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
# Standalone binary acquisition
git -C vendor/saucepan sparse-checkout set resolvers/python

# Python SDK with its matching binary resolver
git -C vendor/saucepan sparse-checkout set sdk/python resolvers/python

# Rust CLI development; cone mode keeps root Cargo files visible
git -C vendor/saucepan sparse-checkout set src tests features openspec/specs
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

For a released version, run the vendored standard-library-only resolver with an explicit tag:

```sh
python vendor/saucepan/resolvers/python/saucepan_resolver.py v0.2.0 .tools/saucepan
```

Use an `.exe` destination on Windows. The resolver downloads directly from GitHub Releases and makes POSIX downloads executable. If the submodule points to an unreleased commit, build that commit instead of pairing it with an unrelated release:

```sh
cargo build --release --manifest-path vendor/saucepan/Cargo.toml
```

Pass the resulting path explicitly to integrations when reproducibility matters.

## Use the CLI or SDK

Create a workspace containing `saucepan.toml`, then invoke:

```sh
<saucepan-binary> <workspace-root> list --json
<saucepan-binary> <workspace-root> install owner/repo --ref <tag-branch-or-sha>
<saucepan-binary> <workspace-root> path <manifest-name>
```

Install the vendored SDK as a local path dependency using the consumer's Python package manager. For an editable development environment:

```sh
python -m pip install -e vendor/saucepan/sdk/python
```

```python
from saucepan_sdk import Workspace

workspace = Workspace("path/to/workspace", binary=".tools/saucepan")
sauce = workspace.install("owner/repo", reference="v1.2.0")
artifact_root = sauce.path
```

Use `Sauce.path` or `Workspace.path()` rather than reconstructing Saucepan's storage layout. Catch `SaucepanError` or its exit-code-specific subclasses for diagnostics.

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

# Python resolver
python -m py_compile vendor/saucepan/resolvers/python/saucepan_resolver.py

# Python SDK
cd vendor/saucepan/sdk/python
uv sync --group dev
uv run pytest -q
uv run behave
uv run behave --dry-run
```

For SDK runtime changes, verify `src/saucepan_sdk` still imports only the Python standard library and its own package. For consumer work, also run the consumer's relevant tests against the pinned binary and submodule commit.

## Hand off

Report:

- selected component paths and why each is needed;
- submodule path and pinned Saucepan commit/tag;
- binary acquisition method and version;
- consumer integration files changed;
- tests run and their results;
- whether fresh clones need a sparse-checkout bootstrap step.
