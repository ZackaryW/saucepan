# Central-index example

This example shows how an index can install a repository that has no root
`sauce.json` of its own.

The example files are:

- [`saucepan.toml`](saucepan.toml), which enables GitHub and standalone index
  fetching;
- [`bucket.json`](bucket.json), which supplies a manifest and the ref it
  describes.

Copy `saucepan.toml` into a workspace, then place `bucket.json` at the root of a
Git repository published as `owner/central-index`. Replace the placeholder
targets with real repositories before running:

```sh
saucepan . bucket add owner/central-index --ref main
saucepan . install owner/plain-script-repo
saucepan . cat sauce widget-cli
```

The install flow is:

```text
owner/plain-script-repo has no root sauce.json
                    ↓
owner/central-index matches the normalized target
                    ↓
checkout v2.0.0 and install manifest.name = widget-cli
```

Updating `widget-cli` later re-reads the registered index. If the winning index
entry changes its `ref` and manifest, `saucepan . update widget-cli` adopts that
current entry and records its newly resolved commit.
