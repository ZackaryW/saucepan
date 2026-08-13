# Artifact formats and workspace layout

## `sauce.json`

A manifest requires `name`, `version`, and `description`. Additional fields are
preserved.

```json
{
  "name": "my-tool",
  "version": "1.2.0",
  "description": "A useful command-line tool",
  "custom_field": "preserved"
}
```

## `bucket.json`

A bucket is an array of entries. Every entry requires `name`, `version`, and
`url`; keeping these fields required preserves compatibility with older
Saucepan binaries. Additional fields remain flattened on the entry and are
preserved verbatim.

```json
[
  {
    "name": "my-tool",
    "version": "1.2.0",
    "url": "owner/my-tool",
    "channel": "stable"
  }
]
```

To supply a manifest for a repository, add both reserved fields:

- `manifest`: the complete manifest document;
- `ref`: the branch, tag, or commit that manifest describes.

```json
[
  {
    "name": "plain-script-entry",
    "version": "2.0.0",
    "url": "owner/plain-script-repo",
    "ref": "v2.0.0",
    "manifest": {
      "name": "widget-cli",
      "version": "2.0.0",
      "description": "Curated by a central index"
    }
  }
]
```

An entry with `manifest` but no `ref` is invalid for manifest resolution;
Saucepan warns and continues the chain. The outer entry's name and version are
search metadata. The nested manifest supplies the installed identity and
version.

## Installed provenance

GitHub and custom-Git index entries include `manifest_source`:

```json
{"kind": "repository"}
```

or:

```json
{"kind": "index", "index": "owner/central-index"}
```

Older entries without this field are treated as repository-sourced. Local
entries do not pass through the manifest-resolution chain and do not carry the
field.

GitHub entries can also contain optional `reference` and `resolved_commit`
fields. Bucket registrations can contain the same pair for repository-target
indexes.

## Workspace layout

```text
<root>/
  saucepan.toml
  .saucepan/
    index.json
    buckets.json
  github/
    owner--repo/
  customgit/
    name/
  indexes/
    repository-target/
```

GitHub targets replace `/` with `--` in managed directory names, keeping
`owner/repo` distinct from `owner_repo`. Local bucket files and `file://` URLs
are read directly and are never copied under `indexes/`.
