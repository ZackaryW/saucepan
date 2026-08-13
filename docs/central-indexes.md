# Central indexes

A central index is a `bucket.json` source that can describe a repository which
does not carry its own root manifest. Saucepan registers indexes in the existing
bucket registry; there is no separate index registry or HTTP client.

## Register an index

A bucket target can be:

- a path to a local `bucket.json` file;
- a `file://` URL;
- a repository target accepted by Git or gh, such as `owner/index`, a Git URL,
  or a local repository directory.

```sh
# Offline registration; no fetch occurs.
saucepan . bucket add owner/central-index

# Resolve and record a branch, tag, or commit immediately.
saucepan . bucket add owner/pinned-index --ref v1.0.0

# Fetch current state and update the recorded commit.
saucepan . bucket refresh owner/central-index
```

If pinned registration cannot resolve its ref, Saucepan rolls the registration
back. Refreshing a local file validates that it can still be read but records no
commit.

## Manifest resolution

After a GitHub or custom-Git target is fetched, Saucepan evaluates an ordered
chain:

1. the configured manifest file at the fetched repository root;
2. registered central indexes, in registration order;
3. not found (exit 1).

The first manifest wins. A repository-owned manifest always prevents indexes
from redefining that project.

An index entry that supplies a `manifest` must also supply the `ref` described
by that manifest. Saucepan checks out the ref before recording the entry, so the
declared version and working tree cannot silently diverge.

## Target matching

Saucepan compares normalized targets for index matching while preserving the
user's original target string as provenance. Matching ignores:

- a trailing slash;
- a trailing `.git` suffix;
- slash versus backslash separators;
- equivalent GitHub forms such as `owner/repo`,
  `https://github.com/owner/repo.git`, and `git@github.com:owner/repo.git`;
- letter case for GitHub-shaped targets only.

Case is not folded for arbitrary local paths or non-GitHub custom hosts, where
case can be significant.

## Precedence and failures

When multiple registered indexes describe the same target, the earliest
registration wins. Every resolution that encounters the collision warns on
stderr and identifies the winning and shadowed indexes.

An index that cannot be fetched or read also warns and is skipped. It does not
turn an otherwise exhausted manifest chain into a source error, and another
reachable index can still satisfy the operation.

## Update behavior

Updating an index-sourced sauce re-reads the current registered indexes. The
winning entry may therefore supply a newer manifest and ref than the one used at
installation time. Saucepan records the newly resolved commit and current
manifest provenance.

Pinned sauce checkouts are normally detached. Saucepan fetches updated refs
without trying to merge into that detached checkout, then checks out the commit
selected by the resolution chain.

## Reads versus registry mutations

`search`, `cat bucket`, and install/update resolution may fetch an index to read
it, but never change the registry's `resolved_commit`. Only these explicit
commands mutate recorded bucket state:

- `bucket add --ref`;
- `bucket refresh`.

See the [central-index example](examples/central-index/README.md) and
[artifact formats](artifact-formats.md) for a complete entry.
