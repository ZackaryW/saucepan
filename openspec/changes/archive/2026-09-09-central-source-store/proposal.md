## Why

Saucepan needs one user-level source store so repeated acquisitions reuse repositories and artifacts. Recipes describe content, policies control caching and verification, and optional authoritative mode gives an application a verifiable sub-index of that store.

This change requires a complete reimplementation from the ground up in a new `src/`. The rejected implementation has been moved to `src3/`; the earlier legacy implementation remains in `src2/`. Both are local-only references, not starting code for the rewrite. Existing tests, commits, dependency lists, schemas, and completion claims do not define the new implementation or establish acceptance.

## What Changes

- **BREAKING** Acquire through one central user store. Identify sources using SHA-256 of canonical inputs; Git identity includes repository origin and requested branch/ref. Different refs have different source identifiers; folders under the same repo/ref do not. Record and verify Git origin consistency on each use. Use SHA-256 for resolved content identification as well.
- Separate declarative acquisition recipes from app cache and verification settings. Implement Git, URL, and local in this rewrite. Git includes repository URLs, directory selection, tracked revisions, explicit pins, and complete required submodule/LFS content. URL additionally supports direct ordinary-file and archive downloads; local acquires content from a local path. URL and local support are required deliverables, not future extension points.
- Keep one current source snapshot separate from at most five historical source ZIPs. Subdirectory requests extract from the same source snapshot; they do not create separate snapshot units, current records, or histories. On a successful changed source acquisition, preserve the outgoing current and publish the new current. Evict history by LRU. Exclude `.git` directories and gitfiles everywhere.
- Use the OS keyring for secret custody and authenticated encryption for the indexes. Retain explicit custom-path/custom-key test construction. Optional repeated content verification defaults off; snapshot retention defaults on.
- Let policy control local-cache fallback when a remote update check fails. When allowed, reuse a suitable recorded local copy and report that updates could not be checked; otherwise fail. This does not advance source current/history or bypass mandatory checks. Fallback is disabled by default: a failed update check returns an error unless policy explicitly allows fallback.
- **Register and configure apps in the central encrypted index.** Store each app's settings and initial filters there; the new app does not load `saucepan.toml`. Operations resolve the registered app record, record the entries it touches, and present only those touched entries matching its filters. Its sub-index is a scoped view of that same index, not a separately distributed index with an issuance/refresh lifecycle. In authoritative mode, the encrypted index records the scopes and corresponding HMAC and the app uses Saucepan's verification method. App identity, scopes, and selected data are authenticated together; scopes do not define resource grants.
- Cache retention, content verification, and fallback are app settings. Apply the calling app's settings without changing another app's settings or creating app-owned snapshot queues. The central source/ref cache remains shared.
- Keep the caller token stable across app-index and settings changes. Saucepan maintains the current scoped-data HMAC internally; the stable caller token and changing data HMAC have distinct purposes. When snapshots are re-enabled, save from the next acquisition and leave gaps for previously unsaved copies without reconstruction or backfill.
- Support central artifact directories and optional independent mirrors. Keep one Saucepan executable at `~/.saucepan/bin`; executable placement does not introduce a runtime manager.
- Target Windows, macOS, and Linux with the same app API, index model, SHA-256 identity rules, and HMAC behavior. Isolate native secret storage and filesystem handling behind platform adapters. Version request/proof formats and reject unsupported versions or authority restrictions explicitly. Cross-platform API compatibility does not imply automatic transfer of tokens or encrypted stores between machines.
- Create a fresh `src/` following the supplied `src-struct` boundaries. Write the core, adapters, and behavioral tests against this change instead of copying, wrapping, importing, or incrementally porting `src2/` or `src3/`. Both reference directories stay ignored and excluded from builds and packaging.

## Capabilities

### New Capabilities

- `acquisition-recipes`: Declarative Git, URL, and local acquisition and complete requested content.
- `artifact-policies`: Independent caching and optional verification settings.
- `encrypted-store`: Keyring-backed authenticated indexes and explicit test stores.
- `application-authority`: Register/configure apps and record their operations in the encrypted index; present and verify the corresponding scoped app view.
- `shared-source-store`: Canonical user-level source reuse and mandatory Git-origin consistency.
- `artifact-cache`: One current source snapshot and five historical source ZIPs with LRU; shared by all folder extractions.
- `artifact-materialization`: Central content directories and explicit independent mirrors.

### Modified Capabilities

- `implementation-structure`: The new recipe/store/scoped-index boundaries replace legacy checkout orchestration and prescribed legacy file organization.

## Impact

Rebuild the Rust acquisition API and its thin CLI adapter from an empty source tree, with newly written tests for the eight capabilities in this change. Re-establish root Cargo targets and dependencies as needed by that implementation; do not point them at either reference tree to keep the rejected app running. Applications, including SDK consumers, call the same sub-index verification method. This is a complete implementation of the revised scope, not a partial adaptation of the previous app. It does not require rebuilding the complete Python SDK, resolver, bucket registry, search system, or legacy command surface.

The source rename is already complete. The fresh implementation began with generic `src/utils` helpers under red-first TDD, then implemented the shared core and thin CLI. Completion evidence is recorded in tasks.md and the current central-store guide; helper tests alone do not establish app acceptance. Existing root tests and historical implementation notes remain superseded evidence, not a compatibility gate. No reference-copy task or second maintained Cargo project is needed.

### Rejected designs: do not reintroduce

These decisions apply to behavior, regardless of its name or where it is implemented. They are not a deferred backlog.

| Rejected idea | Agreed boundary |
| --- | --- |
| Replacing caller tokens after ordinary index changes, or reconstructing unsaved history when snapshots resume | Keep caller tokens stable while Saucepan updates the scoped-data HMAC. Save snapshots from the next acquisition and leave missing history gaps. |
| Restoring app-root TOML settings or using a local configuration file as a fallback/override | App registration and configuration persist settings and initial filters in the central encrypted index. Operations use that registered record. |
| Restricting this rewrite to Git and postponing URL/local sources as extension points | Git, URL, and local are all required source types. Restoring these source types does not require porting the old implementation or restoring unrelated legacy features. |
| Scopes as source/recipe/artifact permissions, dependency allowlists, action roles, or destination grants | Scopes describe how to select an app's sub-index. HMAC verification authenticates that selected data. |
| A separate distributed sub-index, proof issuance/refresh service, or manual selection ceremony for entries the app uses | One Saucepan records each app's touched entries and applies its initial filters; the sub-index is the resulting view of that same index. |
| Treating every ref of a repository as one source identifier, or letting apps overwrite each other's policy settings | Git source identity includes the requested ref. Settings belong to the calling app; shared content does not imply shared app settings. |
| Owner/manager credentials, OS/path identity enrollment/rebinding, per-artifact approval, or extra permission checks during acquisition and before returning results | App registration/configuration is required central-index bookkeeping. Verify the app sub-index before consuming its index data; do not turn registration into a permission-administration service. |
| OS ACL/account/process isolation or treating `.saucepanhash` as an RSA key, publisher signature, or OS identity credential | Trust the OS user; use the OS keyring for secrets and the marker for the app's sub-index proof/reference. |
| Treating folders from the same source as separate snapshot units; folder-specific current copies, retained ZIPs, or history queues | One source identifier (repository plus ref for Git) has one current snapshot and up to five historical source ZIPs. Folder requests at that ref extract from those shared snapshots. |
| Snapshot queues or cache-policy flags owned by installed units; frequency-based eviction or exemptions for installed artifacts | Keep recipes and policies separate. Historical source ZIPs use LRU; extracted content has a separate lifetime. |
| Keeping `.git` in snapshots or taking upstream snapshots from locally edited mirrors | ZIPs contain exported source content with no `.git` directories or gitfiles at any depth. |
| Per-project authoritative source clones, per-app Saucepan versions, or acquired artifact binaries in `.saucepan/bin` | Reuse the user-level source store, create independent mirrors only when requested, and keep one shared Saucepan executable in `.saucepan/bin`. |
| Restoring, wrapping, or incrementally porting either rejected source tree; recovery or reference-maintenance work presented as unfinished implementation | Write fresh `src/`. `src2/` and `src3/` are already preserved, local-only, ignored, and excluded from builds/packages. |

### Outside this change: no implied follow-up work

Legacy-state migration/import, a maintained reference Cargo/test project, key rotation/backup/reset tooling, runtime registration/protocol negotiation/bootstrap/download/update/activation machinery, and release-workflow redesign are excluded from this change. Full Python SDK/resolver/bucket/search/legacy command parity and old compatibility matrices are not acceptance requirements. Unavailable platform runners do not justify expanding the rewrite.

These exclusions do not promise future implementation. The canonical legacy specs and old implementation do not expand the eight capabilities listed above into a compatibility program.

App registration and configuration in the encrypted index are explicitly in scope. The exclusions for runtime registration and permission enrollment do not exclude registering an app's identity, settings, filters, and scoped records.

Read-only app authority and one app issuing tokens for another were discussed as possible later extensions. Do not implement either in this change. Versioned formats and rejecting unsupported restrictions preserve that extension path without adding roles or delegation now. Windows/macOS/Linux adapters and focused validation are in scope; arbitrary OS coverage, a production headless secret-store fallback, cross-machine key transfer, and a legacy compatibility program are not implied.

### Rule against reintroducing rejected work

Before adding or revising a requirement, task, model, helper, test, dependency, or completion claim, compare its actual behavior with the rejected designs and exclusions above. Renaming a permission check to a safety check, a folder cache to an acquisition unit, or a runtime manager to a resolver prerequisite does not make it new scope. If it reproduces excluded behavior, leave it out; do not describe it as newly discovered necessary work.

Old source, tests, schemas, canonical legacy specs, commits, zmem lessons, generated plans, and earlier assistant statements are historical evidence, not authorization to undo a user correction. A conflicting draft passage must be corrected to match the accepted boundary, not used to justify the rejected implementation.

Reopening a rejected design requires an explicit user decision that identifies and changes that earlier boundary. Generic instructions such as "next", "proceed", "continue automatically", or "finish implementation" continue the accepted scope. They do not reopen rejected work. Routine implementation choices within the accepted scope need no additional permission.

Unresolved questions remain visibly unresolved. Do not present an assistant assumption as an agreed requirement or use an open question to revive a rejected design. The user clarified on 2026-09-10 that associated tests, SDK changes, docs, specs, and archived plans are included in commits. Both local reference directories remain excluded from Git.
