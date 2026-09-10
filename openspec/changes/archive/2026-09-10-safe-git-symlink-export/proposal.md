## Why

Remote testing rejected `github/gitignore` at commit `9e86bc12f67365b8dd974d3b3f09d166265c5530` because three committed paths are symlinks. Resolving safe links into ordinary exported content will let these repositories work while keeping snapshots portable and independent of host symlink privileges.

## What Changes

- Resolve relative Git symlinks to files or directories inside the complete committed source tree, then write independent ordinary files/directories at the link paths.
- Resolve links before applying a recipe's folder selection. A link may refer outside the selected folder when its target remains inside the complete exported source tree.
- Reject absolute or escaping targets, missing targets, cycles, Git administration targets, unsupported entries, and expansion exceeding fixed resource bounds before publishing source state.
- Resolve included submodules and LFS content using existing exact-input rules; aliases receive the resolved bytes and executable metadata.
- Retain `.git` exclusion, source/ref identity, shared snapshots, five-entry historical LRU, app settings, and stable token/view verification.
- Add red-first unit and real-Git integration tests plus a repeatable public HTTPS acceptance check for the previously rejected repository.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `acquisition-recipes`: Define safe Git link resolution against the complete source tree before folder selection, with explicit failure and expansion rules.
- `artifact-cache`: Require dereferenced ordinary content in Git snapshots and compatible identity/reuse behavior for resolved aliases.

## Impact

Primary implementation areas are `src/core/sources/git.rs`, a small Git export component, and generic pure resolution helpers under `src/utils/` where appropriate. Existing archive writing, central publication, app authentication, SDK command shapes, and native keyring interfaces remain in place. Tests and the central-store guide need updates. No new runtime dependency is expected.

This change covers committed Git symlinks only. It does not enable local filesystem symlinks, URL ZIP symlink entries, native symlink creation, hardlink output, arbitrary filters/installers, new permissions/scopes, per-artifact policies, or a new index schema. Those are explicit non-goals, not deferred tasks in this change.
