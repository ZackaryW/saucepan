## Why

Saucepan can only install a repository that carries its own root manifest. When the manifest is absent, `fetch_sauce` returns not-found and the install fails — normative behavior, not an oversight. That rule makes sense for artifacts authored as sauces, but it excludes the large and useful case of a repository you did not write and cannot modify: a plain repository whose contents you want to acquire, version, and address like any other sauce.

Buckets already express the idea of describing repositories you do not control, but they are too thin to help. A `BucketStub` carries only a name, version, and URL, and bucket fetching is restricted to local paths — so an index cannot supply a manifest and cannot be shared.

This change lets a manifest come from a **central index** instead of from the repository itself, turning curation into a first-class alternative to authorship. It is additive by construction: it activates only where saucepan currently fails.

## What Changes

- Manifest resolution becomes an **ordered chain** rather than a single check: the in-repository manifest first, then registered central indexes, then not-found exactly as today. The order is specified as a list so further links can be appended later without a breaking change.
- A **central index is a repository target, not a URL**. Registering one reuses the existing Git and gh fetch machinery — no HTTP client, no new dependency, authentication inherited, and the index itself becomes versionable, pinnable, and provenance-bearing.
- `BucketStub` gains flattened extra fields so an index entry can carry a full manifest document and arbitrary consumer data. `name`, `version`, and `url` **remain required**, so indexes written for this version still parse on older binaries.
- An index entry that supplies a manifest **must also supply a ref**, which is checked out during fetch. Version and working tree therefore agree by construction, and index-asserted versions cannot drift from the code they describe.
- Each index entry records the **source of its manifest**, so an installed entry can be traced to the repository or to the index that described it.
- An index that cannot be fetched **warns and is skipped**. An unreachable index never fails an installation that would otherwise succeed.
- **BREAKING** for exit-code feature detection only: a target that previously failed with exit 1 for a missing manifest may now succeed when an index describes it. No currently-succeeding operation changes behavior.

## Capabilities

### New Capabilities

- `manifest-resolution-chain`: The ordered manifest lookup across the repository and registered central indexes, its precedence and termination rules, and the extension point for future links.
- `central-index`: Central indexes as repository targets, the widened bucket entry schema with its backward-compatibility guarantee, the ref requirement on manifest-supplying entries, index precedence, and skip-on-unreachable behavior.

### Modified Capabilities

- `installation-contract`: Installed identity may now originate from an index rather than the repository, so identity resolution gains a manifest-source provenance obligation, and the missing-manifest failure category becomes a chain-exhaustion failure category.
- `python-sdk`: Sauce snapshots gain the manifest-source field, and bucket stubs expose their extra fields to callers.

## Impact

- **Code**: `src/sources/git.rs` (manifest lookup replaced by chain resolution), `src/bucket.rs` (stub schema and remote fetch as a repository target), `src/commands/install.rs` and `src/commands/update.rs` (chain wiring and ref checkout), `src/index.rs` (manifest-source provenance), `src/config.rs` (index registration), `src/commands/bucket.rs` (registering repository-target indexes).
- **Consumers**: existing installs are byte-identical in behavior, because the in-repository manifest always wins the chain. The Python SDK gains additive fields only, and the documented exit-code to exception mapping is unchanged. The vendorable platform resolver is unaffected.
- **Compatibility hazard**: an index that omits `name`, `version`, or `url` would fail to parse on older binaries. Keeping all three required is a normative requirement of this change, not a convention.
- **Downstream**: this change unblocks the `zush` gh-first migration (`c260728-gh-first-zush-migration` in the zush-2 repository), which depends on manifest-less installs and on command trees carried in manifest extras.
- **Dependencies**: none added. Remote index fetching deliberately reuses the existing Git and gh subprocess path rather than introducing an HTTP client.

## Governance Provenance
- Mode: `persistence`
- Intended base: `main`
