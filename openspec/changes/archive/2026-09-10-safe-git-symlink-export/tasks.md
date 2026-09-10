## 1. Pure resolution helpers, red first

- [x] 1.1 Add failing tests for relative file/directory aliases, chains, sibling targets, intermediate aliases followed by `..`, repeated targets, broken links, cycles, root escape, absolute/drive/UNC paths, backslashes, malformed text, and `.git` targets; record the focused red test result before implementation.
- [x] 1.2 Implement the smallest generic logical-tree resolver under `src/utils/`, independent of Git/core/host filesystem I/O; verify the cases from 1.1 pass and legitimate repeated targets are not treated as cycles.
- [x] 1.3 Add failing tests for link-hop limits and expanded entry/byte budgets, then implement bounded expansion with small injected test limits; verify counts include alias duplicates and failures occur before unbounded traversal or allocation.

## 2. Git export integration

- [x] 2.1 Add real Git fixtures using committed mode `120000` blobs without requiring OS symlink creation; verify the current exporter fails the safe-file, safe-directory, and cross-folder acquisition cases before changing it.
- [x] 2.2 Separate committed-tree discovery and ordinary-tree emission in the Git source component, wire in the helpers, and preserve original target context through directory aliases; verify safe fixtures produce exact bytes/executable metadata, including when an alias directory is selected.
- [x] 2.3 Integrate aliases with existing exact submodule and validated LFS expansion; add red-first cases for links into/out of included submodules and to LFS files, then verify correct bytes/dependency records and explicit failure for missing required content.
- [x] 2.4 Verify failure boundaries using an already populated store: unsafe/cyclic/excessive incoming exports leave current/history and app touches unchanged, even with retention or repeated content checks disabled and fallback enabled; verify local-source and URL ZIP links still fail under their existing policies.

## 3. Snapshot and caller regressions

- [x] 3.1 Verify retained ZIP entries, extracted folders, historical reads, and mirrors contain ordinary resolved content with no `.git` or symlink entries; edit an alias copy and verify independent target/mirror bytes, plus content-check rejection of the edited alias.
- [x] 3.2 Verify source/ref identity, shared folder snapshots, unchanged reacquisition, target-change rotation, the five-entry LRU, exact pins, and pre-change link-free snapshot compatibility using deterministic integration fixtures.
- [x] 3.3 Exercise the public CLI and TypeScript/shell adapters with a Git link fixture; verify existing recipe/result shapes, app-specific touched views, stable caller tokens, and view verification still work without new flags or SDK policies.

## 4. Acceptance and documentation

- [x] 4.1 Run `cargo test --locked`, `cargo fmt -- --check`, and `cargo clippy --locked --all-targets -- -D warnings`, plus affected SDK suites; run deterministic Git link tests on Windows and Linux and include them in macOS CI, recording actual execution results separately from configured jobs.
- [x] 4.2 Rebuild the release executable and acquire `https://github.com/github/gitignore.git` at `9e86bc12f67365b8dd974d3b3f09d166265c5530` over HTTPS, including a `Global` selection; verify the three formerly rejected links resolve to exact committed target bytes, ZIPs contain ordinary entries, and source origin/view verification succeeds. Keep this network check separate from default offline tests.
- [x] 4.3 Update the central-store guide and applicable SDK guidance to describe copied Git aliases, source-root resolution, mandatory failure cases, and unchanged local/URL boundaries; verify examples match the implemented CLI and record validation evidence without using legacy reports as acceptance results.
