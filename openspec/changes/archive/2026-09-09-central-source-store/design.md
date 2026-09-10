## Context

See proposal.md for the revised scope. The previous implementation grew around a permission controller and a large compatibility program and is now preserved in `src3/`. The earlier legacy code is in `src2/`. Implement the app from the ground up in a fresh `src/`; neither reference defines its architecture, API, storage schema, or tests. Do not copy modules, wrap the rejected service, or incrementally port either source tree. Previous completion counts are discarded.

The OS user is trusted. The OS keyring protects secrets using the platform service. Saucepan provides consistent shared acquisition and verifiable scoped data through its API, not isolation from another process running as that user.

## Goals / Non-Goals

**Goals:** One reusable source store; declarative acquisition; independent cache/verification policies; complete Git ZIPs; optional app sub-index verification; central content with explicit mirrors.

**Non-Goals:** Permission administration, OS identity enforcement, a runtime manager, migration tooling, full legacy command/SDK parity, and maintaining the legacy reference as a second project. These exclusions also apply to internal helpers and tests: do not rebuild an excluded subsystem under another name.

The [rejected designs and rule against reintroduction](proposal.md#rejected-designs-do-not-reintroduce) are binding design boundaries. Review behavior rather than terminology: a new helper, API, dependency, or test cannot restore a rejected responsibility. Older code, memory entries, and assistant claims do not override these corrections. A generic instruction to continue work leaves these exclusions in force; only an explicit user revision of a boundary changes it. This does not add an approval step for ordinary implementation within the agreed scope.

## Decisions

### App behavior carried forward from the legacy design

The inspected legacy flow loads `saucepan.toml` from the caller's root, records successful install/update results in that root's index, and reads list/path results from that index. This describes the old implementation only. The rewrite replaces TOML settings with app registration and configuration in the central encrypted index. App-specific settings and records remain, but their persistence and lookup are central. The new implementation does not load, require, override from, or fall back to app-root TOML files.

Register the app and store its settings and initial filters in its encrypted index record. Configuration changes update that record. Operations identify the registered app, authenticate the central index, and read its settings there. This applies to ordinary app operations as well as authoritative mode; the authoritative option adds verification of the scoped view. App registration is normal configuration bookkeeping, not owner credentials, OS/path enrollment, or a role/action permission system.

The legacy code is behavioral reference, not new source code: its index was plaintext, URL repository targets used Git, local entries referenced their original paths, and it had no HMAC sub-index service. The new encrypted shared store and the user's corrections govern the replacement; neither old limitations nor previously invented services are acceptance requirements.

### 0. Start with a new implementation

The source rename has already happened: `src3/` contains the rejected implementation. Fresh `src/` work begins with generic utilities and a library module entry for testing; app/core/CLI implementation follows later. Keep `src2/` and `src3/` local, ignored, and excluded from builds/packages. Reading them to understand a past mistake is allowed; bringing their implementation back through copying, imports, includes, dependencies, command dispatch, or fallback is not the rewrite.

Create the new source tree using the directory boundaries supplied by `src-struct`. Write the models, acquisition flow, encrypted store, scoped-index verification, and adapters from this design. Re-establish only the Cargo targets and dependencies needed for those responsibilities. Do not reconstruct a separate runnable reference project or add a placeholder app to claim the build has been restored.

Write behavioral tests from the revised specs as each capability is implemented. Existing root tests, SDK fixtures, compiled binaries, old schemas, and implementation reports belong to the rejected attempt unless independently superseded by new tests. They must not cause excluded behavior or old public API shapes to be reintroduced. Package/build checks must cover both reference directories' exclusion.

Finish the complete helper layer before advancing to core/app/CLI implementation. Its inventory is streaming hashing, sorted serialization, logical/native path validation, bounded native locks, staged file/directory publication, filtered tree copying, ZIP writing/extraction, and generic cryptographic byte operations. Verify these compose end to end. Helpers accept caller-supplied filters, limits, key material, and authentication context; they do not define source IDs, app records, index/token formats, keyring custody, or retention rules. Avoid speculative wrappers around standard/library calls that already provide the required behavior. "Continue automatically" during this phase means finishing these helpers first, not starting app behavior after only the initial four helpers.

Implement `src/utils` first using red-first TDD. Start with domain-neutral streaming hashing, sorted JSON encoding, lexical relative-path validation, and atomic single-file writing. Prefer concise generic inputs (`Read`, `Serialize`, path references, and writer callbacks) and library primitives. Core code decides source identity, schema, key management, filesystem ownership/symlink rules, and policy. Utility tests must demonstrate behavior failures before implementation; a passing library utility suite is not a completed app build.

### 1. Keep the core vocabulary small

| Concept | Responsibility |
| --- | --- |
| Source | SHA-256 canonical identity including the Git ref when applicable, reusable source data, current snapshot, and rolling history |
| Recipe | Git, URL, or local provider; source locator; provider-appropriate content selection and export inputs |
| Artifact | Resolved inputs and resulting content; source/recipe/content identities remain distinct |
| Registered app | App identity, initial filters, settings, and touched-entry associations stored in the central encrypted index |
| App settings | The registered app's retention, content-verification, and local-cache-fallback policies, read from its encrypted record |
| Scope | The app's recorded touched entries together with its initial filtering setup |
| App sub-index | The same central index viewed through that app's scope; verified through its recorded HMAC when authoritative |

An installed artifact does not own a snapshot queue or policy flags. Requesting different folders from one repository extracts different content from the same source; it does not create separate snapshot units. Scopes belong to sub-index construction. Recipes describe content independently of that selection. Source providers produce content and provenance; the store owns reuse and current/history state per source.

### 2. Central placement

Logical layout (directory names beyond the user-selected placement are implementation details):

```text
~/.saucepan/
  bin/saucepan[.exe]              # one shared executable
  index.json.enc                 # sources, registered apps/settings/filters, scopes/HMACs
  sources/
    <sha256-source-identity>/
      repo/                      # canonical repository for Git sources only
      indexes/<state-hash>.json.enc # encrypted source metadata selected by central index
      snapshots/<snapshot-id>.zip  # current plus at most five selected historical ZIPs
      content/<snapshot-id>/     # full source content shared by folder requests

<app location>/
  .saucepanhash                   # optional app proof/reference for its sub-index
  <requested mirror>/            # independent content copy when requested
```

The key is not stored in this tree. Repositories, current content, extracted directories, and temporary preparation are outside the five-history limit. Requests for different folders of the same repository at the same ref share a source identifier, current snapshot, and history. Different Git refs have different SHA-256 source identifiers and therefore independent current/history records; no ref is allowed to replace another ref's current. Underlying Git object reuse is an implementation choice, not a reason to merge those identifiers. Extracted folders do not own separate current snapshots or history queues. Both `src2/` and `src3/` remain ignored and outside root targets/packages; neither needs a recovery or reference-maintenance task.

Implementation detail: current/history are authenticated metadata pointers. Prepare immutable source-index fragments and content before atomically publishing the central index under its lock; authenticate any existing prepared fragment before selecting it on retry. Evict selected historical ZIPs only after publication, and never remove live content directories as ZIP eviction. Old metadata fragments and unselected prepared files can remain; the selected five-history limit is not a total disk quota and does not introduce a general recovery journal. First initialization permits an existing bin-only root so executable placement can precede store creation, while an existing index or native secret prevents reinitialization.

### 3. Authoritative means a verifiable sub-index

```text
App registration/configuration in the encrypted index
           |
           v
Registered app + operation using its stored settings
           |
           v
One Saucepan records the entries touched by this app
           |
           v
Same central index: app's touched entries + initial filters
           |
           v
App's scoped index view
           |
           v
Verify through recorded HMAC when authoritative
```

Scopes describe sub-indexing within the same Saucepan. Record the app context and the entries its operations touch, including when it reuses content already acquired by another app. Present only that app's touched entries that match its initial filtering setup. A global cache hit alone does not make the entry appear for every app. Reading a list must not mark the full central index as touched. The bookkeeping records app operation effects and entry associations; it does not require an unbounded audit log or a new event-processing subsystem.

Scopes do not assign permissions to sources, recipes, artifacts, dependencies, actions, or destinations. Two apps can touch the same source and see it through their own scopes. Each keeps its own settings and operation records while the content is reused centrally.

The full index records the selection scopes and associated HMAC. Bind application identity, scopes, and the selected records into a deterministic authenticated payload. Compute and verify HMAC using a maintained library and a secret available through the store's key provider; a public hash comparison alone is insufficient. Keep index-encryption and sub-index authentication keys domain-separated. The app calls Saucepan's verification API and need not receive the master key or full index.

Maintain the touched-entry associations, scopes, and matching HMAC as part of ordinary Saucepan index operations. The sub-index is the app's current scoped view of that same index; do not introduce a second index synchronization protocol, manual proof issuance/refresh workflow, or independently managed view lifetime. Verification checks that returned data matches its authenticated record. Editing scopes/data or substituting another app's token fails. An app identifier by itself is not proof.

The caller token stays stable across ordinary app operations, settings changes, and changes to touched entries. Establish it with app registration and verify it against the app's stored token binding. Keep that binding separate from the scoped-data HMAC, which authenticates the current app identity, scopes, and selected data and is updated internally with the index. A token authenticating mutable scope/data bytes directly could not remain unchanged as those bytes change, so do not use the same value for both roles. Use separate cryptographic domains for caller-token verification and scoped-data authentication, with store/app binding on the token. A valid stable token does not make stale or altered data valid: Saucepan still verifies the current data HMAC. Operations do not return replacement tokens or rewrite the app marker merely because index contents changed.

`.saucepanhash`, when used, carries the stable caller token/reference for the registered app. It does not embed a changing scoped-data HMAC that the app must replace after operations. It is not the store master secret, an RSA key, an owner credential, or a binding to an OS account/process. Its versioned serialization is an implementation detail of the app API.

An app verifies its sub-index before using that index data for lookup or setup. App-facing index lookups operate on the verified selection: an entry absent from it stays absent, and invalid verification never substitutes a full-index result. This is a data-view contract, not a permission gate on each source, recipe, dependency, or artifact. Shared source acquisition, complete dependency export, caching, and mirrors retain their own contracts without deriving operation grants from scopes. Internal source reuse does not add unselected records to the returned sub-index.

Ordinary app acquisition uses its registered record and settings in the trusted-user store. Registration does not grant roles or require per-artifact approvals. The ordinary API is not a bypass branch inside authoritative verification. The threat model does not claim to prevent the trusted OS user from accessing their own store outside an app's scoped API.

Expose app registration, configuration, app-context index operations, and verification through the same Saucepan core. Registration/configuration persists app settings and initial filters in the encrypted index. Do not add inspect/setup/update/mirror/remove/manage roles, a separate owner marker, destination-grant administration, OS/path identity enrollment, or process authentication. Checking a requested path for safe writes remains ordinary filesystem correctness.

### 4. One acquisition flow

Git, URL, and local are all implemented source types in this change. Git acquires repository content, including Git URL targets. URL additionally supports direct ordinary-file and archive downloads, as explicitly confirmed after inspecting src2. Local reads content from its path into the central store. All three use the common encrypted index, SHA-256 identification, app settings, and content/mirror flow. Git origin and commit rules apply to Git targets; do not invent Git repositories or commits for direct downloads or local non-Git content. Local acquisition does not modify the user's original source path.

This restores the requested source coverage, not the old code or every legacy command. URL and local providers must have working acquisition and behavioral tests before this rewrite is complete. An enum variant or unsupported-provider error is not implementation of either provider.

```text
Registered app identity + recipe
  --> open and authenticate the index
  --> resolve the app record and read its stored settings and initial filters
  --> verify the app sub-index when authoritative input is supplied
  --> resolve app-facing index lookups from the verified selection when supplied
  --> validate the acquisition recipe
  --> check source identity, including actual Git origins for Git sources
  --> acquire Git, URL, or local content using the provider's inputs
  --> prepare or reuse the complete source snapshot and validate the requested folder
  --> preserve outgoing source current when required; publish source current/history
  --> record the app's operation and touched entries with consistent scope/HMAC data
  --> extract the requested folder; return or mirror its content when requested
```

Tracking acquisition attempts to refresh its remote before deciding whether current content is reusable. If the update check fails, policy determines whether a suitable recorded local copy may be used. With local-cache fallback allowed, return the cached revision and explicitly report that the update check failed and freshness is unknown. With fallback disallowed, or without a suitable complete local copy for the requested source and content, fail. Never report fallback content as freshly checked or latest, advance source current, or add/evict history entries because of the failed check. Ordinary cache-read recency updates still apply. Fallback is disabled by default: a failed update check returns an error unless the caller explicitly enables local-cache fallback.

Fallback affects remote availability only. Index authentication, recorded Git-origin checks, app sub-index verification when applicable, complete required content, extraction safety, and policy-requested content checks still apply. An integrity or verification failure is not a reason to retry through an unchecked cache path. Snapshot retention and permission to use existing local content are independent policy choices.

An explicit pin or historical artifact read uses its exact recorded inputs and does not advance tracking; fallback never substitutes a different revision for an explicit pin. Git submodules use exact gitlink commits; required LFS data must be actual validated object bytes. Complete the artifact or fail, without repository-defined installer/filter programs.

Normalize source identities once and hash their canonical inputs with SHA-256. For Git, include the repository origin and requested branch/ref; retain the resolved commit separately so an advancing branch keeps its source identifier. Different folders at one repo/ref do not change that source identifier. Use SHA-256 content hashes to identify acquired content, including URL/local content; content hashes do not replace source provenance or the HMAC. Stored canonical encodings are not reparsed as transport URLs. Authenticate source records and compare configured/effective Git origins on every Git use, including cache hits and scoped reads. First use records the source and the calling app's touch during the normal operation; no manual enrollment list is required.

### 5. Rolling history and verification

Retention, repeated content verification, and local-cache fallback are app settings. Defaults are retention on, repeated content verification off, and fallback off. Use the calling app's settings for its operation; do not overwrite another app's settings or introduce a global policy vote when apps share a source. Disabling retention for one app does not delete snapshots already retained through another app. Snapshot queues remain attached to source identifiers, not apps or installed artifacts.

For a source identifier current at F with history A-E, an app acquisition of G with retention enabled stores F in history when F is available, publishes G as current, and evicts the least recently used historical entry. If A is least recently used, the result is history B-F and current G. An unchanged source resolution reuses current without duplicate history. Requesting another folder at that same source revision only extracts from the shared snapshot; it neither creates another current snapshot nor consumes a history slot. History reads, including folder extraction from history, refresh that source snapshot's recency.

Snapshots contain the source's complete exported tree, with paths relative to the repository root. Exclude `.git` directories and gitfiles at every depth, including submodules. Preserve ordinary exported files, including `.gitignore`. A folder request extracts only that folder's contents into its output root; the retained ZIP remains the shared source snapshot. Record source revision, export inputs, and content digests independently of requested folder paths. Extraction results identify their source snapshot and folder. When optional verification is requested, compare consumed bytes with recorded digests. Mandatory index authentication, origin checks, required LFS digest checks, and extraction containment remain active regardless of that option.

With an app's retention off, produce the same artifact without adding retained ZIPs for that operation. Keep resolved source-current metadata and preserve already retained history. Changing that app setting does not change another app's setting. When it is re-enabled, save the next successful acquisition as current and leave gaps for older content that was never saved. Do not reconstruct, backfill, or fail solely because an outgoing copy was never retained. Preserve an already retained outgoing current through the normal rolling rule.

For example, if F was acquired while retention was off and never saved, enabling retention and acquiring G saves current G without inventing a historical F or evicting an old history entry just for the gap. The next advance from G to H can retain G normally. If the first acquisition after re-enabling still resolves to F, save F as current then; unchanged source inputs do not excuse leaving current ZIP storage empty once retention is on.

Use bounded locks and staged publication sufficient to preserve the last consistent current/history on failure or concurrent access. The contract is the observable outcome; it does not mandate a general transaction framework or a fixed multi-generation journal design. Eviction removes ZIP cache entries, not live artifact directories or source data needed by referenced content.

### 6. Materialization and boundaries

Materialize centrally by default. An explicit mirror is an independent copy at the requested location. Never overwrite unowned files or turn locally edited content into an upstream snapshot. Reject escaping paths and unrepresentable archive entries before publishing the directory. Removing a recorded artifact or mirror affects only its recorded files and references.

Follow `src-struct`: CLI adapters call the core; models describe data; policies evaluate explicit inputs; sources handle acquisition; utils contains small domain-neutral helpers. Index encryption, snapshots, and sub-index verification belong to the core, not CLI argument parsing. One vertical acquisition example should drive the implementation and its tests.

### 7. Shared behavior across Windows, macOS, and Linux

Keep registration, app settings, touched-entry filtering, source/ref identification, snapshots, and HMAC verification in the shared Rust core. Use small adapters for OS secret storage, home-directory discovery, filesystem locks, file replacement, and representable paths/links. Build an executable for each supported target; the API and recorded data rules remain the same.

| Platform | Production secret provider | Shared executable under the detected user home |
| --- | --- | --- |
| Windows | Windows Credential Store | `.saucepan/bin/saucepan.exe` |
| macOS | Keychain | `.saucepan/bin/saucepan` |
| Linux | Secret Service | `.saucepan/bin/saucepan` |

The OS service holds Saucepan's secret; it does not store the entire source index or implement app scopes. Use maintained platform backends rather than custom OS security management. Select only needed dependencies during the fresh implementation; the rejected Cargo manifest does not establish backend support. The [keyring backend documentation](https://docs.rs/keyring/latest/keyring/cli/index.html) describes the native stores; its [library guidance](https://docs.rs/keyring/latest/keyring/) distinguishes the shared keyring core from selectable store implementations.

Production opening requires the configured native provider. A locked/unavailable store or a missing Linux Secret Service returns an explicit error; do not silently choose a plaintext/file key or the custom-key test mode. Headless environments can exercise the same core with explicit custom-path/custom-key test construction, but that is not an automatic production fallback.

Canonical source/ref inputs, SHA-256 content identification, and HMAC payload serialization must be deterministic across supported platforms. Use logical source and archive paths in authenticated data rather than OS-native display strings. Preserve local-source path semantics; this does not promise that different machines' local paths identify the same source. Equivalent logical inputs and key bytes must produce the same identification and authentication test vectors regardless of OS.

Materialization translates logical paths through the platform filesystem adapter. Reject entries that cannot be represented without loss, escape, or collision (including case collisions on the target filesystem) rather than silently renaming or overwriting. Preserve the committed index/content state if locking or replacement fails. Platform correctness does not require custom ACLs or permission enrollment.

Version the request/proof envelope and persisted index format. An unsupported version or authority restriction is an explicit error, never silently interpreted as unrestricted authority. Read-only restrictions and delegated token issuance remain future extensions; implement no such authority administration now. Tokens are bound to their store's authenticated record and secret; API compatibility across OSes does not export keys or make a token valid in an unrelated store.

Use identical core fixtures and cryptographic vectors with custom keys across Windows/macOS/Linux, then verify native credential access and filesystem publication on each supported target. Record actual target coverage and any untested environment; the old platform workflow/tests are not evidence for the fresh implementation. These focused checks do not mandate a large CPU/OS compatibility matrix or runtime-release redesign.

## Implementation details

The previously open behavior decisions are settled: caller tokens stay stable while Saucepan updates the data HMAC, and snapshot retention resumes from the next acquisition without filling unsaved history gaps. Direct file/archive URL downloads are included alongside Git and local acquisition. Document the supported archive formats explicitly; SHA-256 identification does not imply support for every format. Concrete serialization fields and backend library choices must preserve these contracts and the rejected-design boundaries.

## Risks / Trade-offs

- Optional repeated content checks trade detection of local byte changes for speed; report whether checking occurred and never redefine the baseline from unchecked bytes.
- A shared repository with independent artifact/mirror directories uses space outside the five ZIP-history slots; expose this accurately in examples.
- HMAC verification proves consistency with Saucepan's secret-backed record, not publisher identity or protection from the trusted OS user.
- The rejected implementation and its passing tests can bias the rewrite toward the wrong architecture; implement and test from the revised contracts rather than porting those modules or treating old tests as requirements.

There is no legacy migration or automated runtime rollout in this change. The planning step preserves the reference trees without creating replacement code. Subsequent implementation starts with a new `src/`; it does not reconcile the rejected source into compliance. Old stored data, command compatibility, and previous build artifacts are not migration targets.
