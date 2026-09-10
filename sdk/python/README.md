# Saucepan Python SDK

The SDK is a standard-library-only Python client for the Saucepan command-line
interface. It requires Python 3.9 or newer and an independently acquired
`saucepan` executable; it never downloads or builds the executable itself.

## Construct a workspace

`Workspace` is the entry point. Its first argument is the directory containing
the workspace's `saucepan.toml`. By default, construction resolves an executable
named `saucepan` through `PATH`:

```python
from saucepan_sdk import Workspace

workspace = Workspace("/path/to/workspace")
```

Pass `binary=` to use a particular executable path verbatim instead:

```python
from pathlib import Path

workspace = Workspace(
    Path("/path/to/workspace"),
    binary=Path("/path/to/bin/saucepan"),
)
```

A missing default or explicit executable raises `SaucepanError` with a message
that names the executable. Binary acquisition remains the caller's
responsibility.

## Workspace, Sauce, and Bucket

The public model is rooted at a `Workspace`:

- `workspace.sauces` yields `Sauce` snapshots. Each exposes `name`, `version`,
  `description`, optional `reference` and `resolved_commit` values, and
  `manifest_source` — a mapping reporting where the entry's manifest came
  from: `{"kind": "repository"}` or `{"kind": "index", "index": "<registered
  index target>"}`. An entry written before this field existed has no
  `manifest_source` key at all; the SDK treats that the same as an explicit
  `{"kind": "repository"}`. A sauce can `update()` or `uninstall()` itself,
  and its `path` property returns a `pathlib.Path`.
- `workspace.buckets` yields `Bucket` entities. Each exposes its `url`, can
  return parsed stubs with `stubs()`, and can `remove()` itself. Each stub is
  a `BucketStub` — a dict of the raw parsed entry that also exposes `name`,
  `version`, and `url` as attributes, plus `extra`: a mapping of every field
  beyond those three, empty (never missing) when the entry carries none.
- `workspace.install(target, reference=None)` installs a target and returns the
  resulting `Sauce` entity.

For example:

```python
installed = workspace.install("owner/my-tool", reference="v1.2.0")
print(installed.name, installed.version, installed.path)

installed.update()     # refreshes this same object in place
installed.uninstall()

for bucket in workspace.buckets:
    print(bucket.url, bucket.stubs())

pinned = workspace.add_bucket("owner/central-index", reference="v1.2.0")
workspace.refresh_bucket(pinned.url)
```

### Command methods

Every Saucepan CLI read or mutation has an SDK entry point:

| Operation | SDK API |
|---|---|
| Install | `workspace.install(target, reference=None)` |
| Update or uninstall | `sauce.update()`, `sauce.uninstall()` |
| List installed entries | `workspace.list()` |
| Resolve an installed path | `workspace.path(name)`, `sauce.path` |
| Search registered buckets | `workspace.search(jq_filter)` |
| Add, refresh, remove, or list buckets | `workspace.add_bucket(url, reference=None)`, `workspace.refresh_bucket(url)`, `workspace.remove_bucket(url)`, `workspace.list_buckets()` |
| Remove or inspect one bucket | `bucket.remove()`, `bucket.stubs()` |
| Read the full index or registry | `workspace.cat_index()`, `workspace.cat_buckets()` |
| Read one sauce or bucket document | `workspace.cat_sauce(name)`, `workspace.cat_bucket(url)` |

Read methods return parsed JSON values rather than command output text.
`search()` returns a list of the raw parsed values emitted for the caller's jq
filter and requires the CLI's configured `jq` executable to be available.

`Sauce.path` deliberately delegates to the CLI `path` command. The SDK does not
encode or reconstruct Saucepan's on-disk repository layout.

## Exceptions and diagnostics

All command failures derive from `SaucepanError`. The documented CLI exit codes
map to distinct subclasses:

| Exit code | Exception |
|---:|---|
| 1 | `NotFound` |
| 2 | `SourceError` |
| 3 | `ConfigError` |
| 4 | `Conflict` |
| 5 | `InternalError` |

Each raised command exception exposes its integer `exit_code` and the complete
captured standard-error text as `stderr`. Catch the base class for common
handling or a subclass for a particular category:

```python
from saucepan_sdk import NotFound, SaucepanError

try:
    path = workspace.path("missing-tool")
except NotFound as error:
    print(error.exit_code)  # 1
    print(error.stderr)
except SaucepanError as error:
    print(error.exit_code, error.stderr)
```

Executable-resolution failures occur before a command runs, so their
`SaucepanError.exit_code` is `None`.

## Caching and refresh

`Workspace` lazily caches the parsed sauce index and bucket registry
independently. Consecutive reads of `workspace.sauces` reuse one `cat index`
result, while consecutive reads of `workspace.buckets` reuse one `cat buckets`
result.

Every SDK mutation invalidates both caches: install, update, uninstall, bucket
add, bucket refresh, and bucket remove (including their entity-level forms). The
next collection read therefore observes the mutated state. In addition,
`sauce.update()` reads
that sauce's new entry and replaces the fields on the same `Sauce` object, so
the caller does not need to reacquire it or refresh explicitly:

```python
sauce = workspace.sauces[0]
old_version = sauce.version
sauce.update()
print(old_version, "->", sauce.version)  # refreshed on this same object
```

Changes made outside this SDK are not detected automatically. Call
`workspace.refresh()` to discard both caches and immediately reread the complete
sauce index; the bucket registry is reread on its next access.

## Runtime dependency boundary

The runtime package must remain independently vendorable. It imports only the
Python standard library and its own modules; `pyproject.toml` declares no runtime
dependencies. Binary acquisition is managed outside this repository. Supply the
SDK with an independently acquired, compatible CLI executable.

The import constraint was reviewed on 2026-07-28 with an executable AST walk of
every `src/saucepan_sdk/*.py` module. Relative imports were classified as local
package imports; every absolute top-level import was checked against
`sys.stdlib_module_names`. All five modules passed. The reviewed standard-library
imports are `json`, `os`, `pathlib`, `shutil`, `subprocess`, and `typing`; all
other imports are relative within `saucepan_sdk`. Reviewers should repeat this
check whenever a runtime import is added.

The walk was repeated on 2026-07-28 after adding `Sauce.manifest_source` and
`BucketStub` (with its `extra` mapping): the same five modules were walked, no
new absolute import appeared, and the reviewed standard-library set is
unchanged (`json`, `os`, `pathlib`, `shutil`, `subprocess`, `typing`); the two
new attributes are built entirely from data already carried on parsed
entries.

## Vendor only the Python SDK

Git submodules point to whole repositories, not individual directories. Add the
Saucepan repository as a submodule, then use cone-mode sparse-checkout inside
that submodule to materialize only `sdk/python/`:

```sh
git submodule add --name saucepan https://github.com/ZackaryW/saucepan.git vendor/saucepan
git -C vendor/saucepan sparse-checkout init --cone
git -C vendor/saucepan sparse-checkout set sdk/python
```

The vendored SDK is then available at `vendor/saucepan/sdk/python/`.

The sparse-checkout configuration is local and uncommitted. It is not recorded
in `.gitmodules`, and a fresh `git clone --recursive` does not carry it over.
After a fresh recursive clone, each consumer must reapply the sparse checkout:

```sh
git -C vendor/saucepan sparse-checkout init --cone
git -C vendor/saucepan sparse-checkout set sdk/python
```
