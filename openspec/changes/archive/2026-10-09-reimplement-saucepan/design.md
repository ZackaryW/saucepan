# Design

## Context

See proposal.md for motivation. This repository contains OpenSpec configuration and source placeholders, with no Cargo package or existing main specs. The supplied directories are `cli/`, `core/{index,policy,source,store}/`, `models/{index,policy,source,store}/`, and `utils/source/`; `utils/source/git.rs` is an empty placeholder. `mock/` is the designated simulated application root. The sibling ../saucepan is a Rust CLI/library reference, not an implementation base. Its models embed sources and snapshots, use a shared FORMAT_VERSION, and store app touched-artifact sets in an encrypted central index. Its acquisition path prepares sources on tracked requests, and its retention code keeps five historical ZIPs by LRU while preserving live content. Its schema tests reject recipe policy overrides. These are observed behaviors that this change deliberately replaces.

Confirmed decisions are separate source/index/store roots, natural output contents, multiple acquired versions, request-bearing application claims, acquisition-level policies, latest-ten default retention, 24-hour discovery caching, GitHub releases without cloning, and a shim with a custom `useable_app` policy. The earlier flat component graph proposal was superseded by direct filesystem lookup.

## Goals / Non-Goals

**Goals:** A new typed Rust core shared by CLI and shim-related operations; independently managed source and output lifecycles; predictable policy evaluation; recoverable publication; a self-hosted Saucepan release installation.

**Non-Goals:** Legacy schema/format-version inheritance, store migration, old SDK protocol compatibility, a graph runtime, a background refresh daemon, arbitrary installation scripts, and treating app UUIDs as authentication credentials. Ports of old SDKs and old encrypted-view/token protocols are separate work; retaining claims does not silently inherit those protocols. The initial trust boundary is the local OS user, with private filesystem permissions and no secrets in shareable source definitions.

## Decisions

### 1. Direct filesystem ownership

```text
.saucepan/
  source/
    <source-uuid>.json
    <source-uuid>/             # provider-owned working data, if needed
  index/
    <app-uuid>/index.json
    <entry-uuid>/index.json
  store/
    <entry-uuid>/              # natural output contents
  bin/
    saucepan[.exe]            # stable launcher, not the managed application
```

Use globally unique UUIDs and record-kind tags to distinguish application and output indexes. Folder UUIDs are authoritative; reject any conflicting embedded identifier. Source records identify provider and locator, selector-keyed discovery results, last successful check, and provider validators. Only providers requiring working data create source directories. A GitHub release source fetches metadata/assets and never clones the repository. A `github_repo` source maintains a reusable Git repository. Generic Git, URL, and local providers retain their useful roles from the reference implementation under the new protocol.

No mandatory `items/`, no interpretation of package-owned index.json files, and no implicit behavior derived from ingested folder names. Archive handling is explicit: retain a raw asset or extract ZIP content into the output root. Do not put Saucepan bookkeeping inside package contents. Additional formats can be added separately; unsupported formats fail explicitly.

A root inventory can be reconstructed by scanning index directories. Start without another authoritative inventory. Typed Serde records and direct UUID lookup suffice; serde_json is the initial decoder, with alternative parsers requiring evidence from full-load benchmarks. No petgraph dependency is needed.

### 2. Request and identity contracts

An acquisition request contains a source UUID, a tracked release/ref or exact revision selector, ordered platform/asset filters, content selection (folder, archive mode, binary paths), and policies. The source definition owns the provider locator. App identity is optional for unscoped acquisition; successful scoped acquisition records a claim. No user-provided relative path can escape its owning root.

An output index records source UUID, normalized request, resolved provider identity (Git commit, release and asset identifiers, URL validators/digest, or local content digest), content digest, creation sequence, availability state, and effective policies. App indexes contain settings and claims with a claim UUID, original normalized request, and selected entry UUID. Claim IDs distinguish multiple requests by one app. Updating a claim replaces its selected entry only after successful acquisition; history entries remain independently indexed.

A request's item key is the canonical source + selector + selection, excluding the resolved version and lifecycle policies. This is the proposed retention scope: each logical requested item has its own history across versions. The earlier scope question was unanswered; this draft chooses per item to prevent different platforms/folders from evicting one another. Policy overrides are item-specific and persist with the claim/output rather than changing source refresh state.

An exact-output reuse fingerprint additionally includes resolved content identity and effective output lifecycle policies. This avoids one caller silently changing another caller's policy on a shared entry. Compatible requests can share; incompatible policies produce separate entries. Default identical-pull behavior is proposed as `reuse`; explicit `new_entry` always assigns a new output UUID, even if bytes match. Physical deduplication is unnecessary initially. Reclaimed entries are never returned as reusable live outputs.

### 3. Source refresh and selection

The default discovery TTL is 86,400 seconds from the last successful check, evaluated on use. Reads do not extend it. Check immediately when absent/expired; a failed check does not advance the success timestamp. A backward wall-clock jump invalidates freshness rather than extending it indefinitely. Record stale fallback explicitly when requested; default to an error on failed due checks. A force-refresh option bypasses TTL. Exact entry reads need no latest-release check.

Cache discovery by source and release/ref selector, not platform filter. Thus Windows/Linux requests can filter the same cached release metadata. Git uses shared repository objects but resolves the requested ref; immutable output exports use exact commits. Local sources inspect current local content on acquisition; they have no remote-discovery TTL. URL sources use validators where available and record byte digests. Cache metadata freshness does not imply downloaded bytes are present or intact.

`github_release` supports latest stable and explicit tag selection. Platform matching uses explicit OS/architecture fields and ordered asset-name patterns, not guessed filenames. Preserve `selection_policy`, `policy_target: github_release_asset_filter`, and `strategy: first_match_win` within the request's typed selection configuration. The first applicable filter yielding assets wins. For the initial single-asset request contract, zero matches fail and multiple assets from the winning filter fail as ambiguous; filter precedence is not arbitrary asset precedence. Binary paths are validated inside the resulting content. Unsupported policies and invalid type combinations fail before download.

### 4. Outputs, claims, and reclamation

Published output bytes are immutable. Source fetches never rewrite them. Ready, reclaim_pending, and reclaimed are distinct availability states; app views report unavailable claims truthfully and path lookup never returns a known missing path as usable. Reclaim keeps request/provenance metadata so an application can explicitly reacquire. Reacquisition creates or reuses a live entry and updates the claim, not the old immutable output's meaning.

The default eviction policy is `latest` with `keep: 10` per item key, ordered by committed creation sequence, not last access or upstream version strings. Reuse does not become a new entry or reorder history. An item's acquisition override selects its own positive retention count. When multiple live requests in the same item scope specify different counts, use the maximum retained window demanded by their attached policies; report effective retention rather than silently applying last-writer-wins. Ordinary claims do not pin entries. Custom protection and in-use leases can defer reclaim, so physical count can temporarily exceed the target and must be reported honestly.

Reclaim marks an entry unavailable to new readers before deleting its content, then records reclaimed after successful deletion. It never removes source data. A failed deletion remains reclaim_pending and is retried. Cross-process leases protect active users against deletion races, particularly executable launches. Metadata tombstone compaction is separate future work; ten is an output-content retention target, not a total metadata or disk-byte quota.

### 5. `useable_app` and self-management

Retain the user's spelling `useable_app` as the policy identifier. It layers activation protection over ordinary retention, applies to a declared executable binding, and is not hard-coded to Saucepan's repository. The app record owns the active entry UUID and executable-relative path. A shim reads that binding without invoking the managed executable to locate itself. It forwards arguments, standard streams, exit status, and appropriate process interruption behavior.

Acquire, validate platform/executable/content, and probe the new executable before changing the active binding. For Saucepan the probe is a bounded version invocation with a known response contract. Until activation succeeds the previous target remains protected. After the atomic binding switch the new target is protected; the old target becomes reclaimable once its running-process leases finish. Never substitute the newest downloaded entry merely because its timestamp is newer. Failures preserve the old active target and diagnostics. Rollback can select a retained, validated previous entry; unavailable entries require reacquisition.

Bootstrap installs the stable shim and an initial verified release output, then registers Saucepan's own application claim and binding. This is a `github_release` acquisition only. Running outputs are not overwritten, avoiding Windows executable replacement constraints. The launcher itself is outside the output cache; launcher replacement is a separate maintenance operation, not part of routine self-update. OS-specific launch/lease behavior needs Windows, Linux, and macOS verification; no platform is claimed validated without execution evidence.

### 6. Publication and recovery

Use bounded provider/source locks for refresh work and a short store mutation lock for committed index changes. Stage on the same filesystem outside final content directories, verify complete content, then publish a new directory and atomically replace its metadata. A journal records multi-record claim/activation transitions so recovery completes or rolls back them without exposing a claim to partial output. A prepared but unreferenced output is safe to reclaim later. Recovery runs before mutations; the shim reads only committed active bindings and coordinates its lease with reclaim. Deduplication must recheck under the publication lock for concurrent identical pulls.

Keep generic helpers limited to the portable mechanics below. The multi-record journal, freshness decisions, and ordered selection/eviction policy evaluation belong to core application modules, not utilities. The CLI is a thin JSON adapter over the same core operations: initialize, register app/source, acquire/update claim, inspect/path, reclaim, activate, and self-update. Isolated roots and fixture servers keep tests away from production data.

### 7. Package structure follows the supplied placeholders

Use one Cargo package with a library, a `saucepan` CLI binary, and a `saucepan-shim` binary. Do not create a workspace of utility crates or scaffold empty modules during planning. Add files as their checkpoint delivers behavior.

```text
src/
  lib.rs                       # public domain API and models
  main.rs                      # delegates to cli
  bin/saucepan-shim.rs          # minimal launcher entrypoint
  cli/                         # args, JSON IO, command dispatch, errors
  models/
    source/                    # provider definitions, resolved metadata
    index/                     # apps, claims, bindings, index record kinds
    store/                     # acquisition requests/results, output state
    policy/                    # refresh, selection, reuse, eviction settings
  core/
    mod.rs                     # explicit root/context and facade
    source/                    # GitHub/Git/URL/local resolution and refresh
    index/                     # persistence, claims, binding lookup, recovery
    store/                     # acquire, stage, verify, publish, reclaim
    policy/                    # ordered selection, retention, useable_app
    activation.rs              # probe, binding switch, rollback
    launch.rs                  # binding + lease + process orchestration
  utils/
    source/git.rs              # generic Git repository operations
    path.rs                    # portable relative-path checks
    fs.rs                      # staged directory and atomic file mechanics
    lock.rs                    # bounded native shared/exclusive guards
    tree.rs                    # bounded tree copy/export mechanics
    archive.rs                 # bounded, staged ZIP extraction
    process.rs                 # bounded child execution/probes
tests/
  utilities/                   # domain-neutral filesystem/process fixtures
  support/                     # fixture HTTP server, clock, isolated roots
  ...                          # domain and CLI integration tests
```

`models` contains data and structural validation, no IO or CLI behavior. `utils` imports neither `models`, `core`, nor `cli`, and accepts caller-supplied paths/values rather than discovering `.saucepan`. `core` composes models, utilities, and direct library calls; cross-area operations are coordinated by the facade/activation modules instead of cyclic ownership. `cli` translates inputs and invokes core; the shim uses the narrow launch API without recursively spawning Saucepan to locate its target. Provider-specific release DTOs stay in `core/source` unless part of the documented public contract. Tests must be wired into Cargo discovery, including nested utility modules.

### 8. Reusable utility plan

This section is the output of `pspec-plan-utilities`. Contracts below are proposed Rust signatures, not implemented APIs. All helper tests use generic filenames, repositories, byte streams, and processes without Saucepan records. Helpers remain local under `src/utils`; no separate reusable package is introduced.

**Reuse assessment.** There is no current Cargo manifest or installed dependency set in this repository. The reference has suitable small filesystem/path/archive helpers, but its Git provider imports application models and cannot be copied wholesale into utils. Its contracts are evidence for a fresh implementation, with portable portions adapted where appropriate, not a dependency on the old application. Inspected local Rust is 1.98.1. Select an explicit MSRV and compatible dependency versions during package setup and lock them in Cargo.lock. The standard library and dependency public APIs below provide most mechanics; add adapters only for the stated gaps. Initial choices avoid libgit2's native build surface and an async runtime; verify crate licenses and enabled features in that setup checkpoint.

**U1 - Relative paths and bounded trees (`utils/path.rs`, `utils/tree.rs`).**
- Purpose/portability: select and copy content beneath an explicit root; equally useful to an asset exporter or backup program.
- Contract: `relative_path(value: &str) -> io::Result<PathBuf>` and `copy_tree(source: &Path, destination: &Path, limits: TreeLimits) -> io::Result<TreeSummary>`. Require a stable source and a new private destination; reject absolute/parent/drive/UNC/alternate-stream paths and host-invalid names. Count files/bytes and detect collisions; local copy rejects links/reparse points rather than following them implicitly. Git link materialization is an explicit repository-export behavior in U5, not an accidental filesystem traversal.
- Effects/failures: reads source and writes only private destination; errors can leave private staging, never publish it. Lexical checks alone are not a sandbox against hostile concurrent root replacement; callers own trusted roots and use component metadata checks.
- Reuse: direct `Path::components`, `symlink_metadata`, `read_dir`, and file IO; adapt the reference `utils/path.rs` and `utils/tree.rs` contracts. `Path::join` alone provides no containment or bounds, so a small checked traversal is justified.
- Verification: nested files/empty directories, Unicode, binary bytes, limits, Windows collisions, links/reparse points, traversal, source changes/failures; observe real files and no outside writes.

**U2 - Single-file replacement and directory publication (`utils/fs.rs`).**
- Purpose/portability: publish a completely prepared file/directory; useful for a configuration editor or build artifact exporter.
- Contract: `atomic_write(path: &Path, write: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()>` and `staged_directory(path: &Path, prepare: impl FnOnce(&Path) -> io::Result<()>) -> io::Result<()>`. Caller supplies an existing trusted parent and serializes competing writers; directory destination must be absent.
- Effects/failures: sibling staging, sync file contents before replacement, retain old file on producer failure, clean abandoned staging where possible. No multi-file transaction, lock acquisition, or universal power-loss directory durability is implied. Core/index owns journal records and replay decisions.
- Reuse: adapt reference `utils/fs.rs`; [tempfile 3.27 `new_in` and `persist`](https://docs.rs/tempfile/3.27.0/tempfile/struct.NamedTempFile.html#method.persist) supply sibling temp files and atomic replacement but do not sync file/directory data themselves. Use `File::sync_all` and report platform-specific durability limits; do not invent an atomic-write framework.
- Verification: old/new bytes under concurrent readers, callback error, occupied directory, missing parent, failed replacement and staging cleanup in isolated filesystems.

**U3 - Bounded native lock guard (`utils/lock.rs`).**
- Purpose/portability: coordinate cooperating processes with a timeout; usable by any local cache or file-backed tool.
- Contract: `LockGuard::acquire(path: &Path, mode: LockMode, timeout: Duration) -> io::Result<LockGuard>`. Guard owns a file handle; release on drop; no PID-based lock ownership. Keep lock files stable rather than unlinking them while another process could hold the old inode.
- Effects/failures: open/create caller's lock file, monotonic bounded wait, distinguish contention timeout from IO failure; no choice of lock ordering or entry eligibility inside utility.
- Reuse: [standard `File::try_lock`, `try_lock_shared`, and `unlock`](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock), available since Rust 1.89, cover native mechanics at the inspected toolchain. Add timeout/RAII only; do not add fs2 merely because the old application uses it. Supporting an older MSRV would require revisiting this choice.
- Verification: two real processes, shared readers/exclusive writer, timeout, process exit, and drop release. Core launch tests separately verify leases remain owned for the target process lifetime, including unexpected launcher exit; a parent-only guard is not sufficient proof.

**U4 - Bounded ZIP extraction (`utils/archive.rs`).**
- Purpose/portability: safely extract an archive into a new directory, useful for a downloader or installer unrelated to Saucepan.
- Contract: `extract_zip(reader: impl Read + Seek, destination: &Path, selection: Option<&str>, limits: ArchiveLimits) -> io::Result<ExtractionSummary>`. Selection and size/count bounds are supplied; caller chooses integrity expectations and retention.
- Effects/failures: stream bounded decoded bytes, reject collisions, unsupported compression, unsafe paths and ZIP links, preserve supported executable metadata, and publish only a complete staged result using U1/U2. Account for malicious input beyond only selected entries.
- Reuse: use `zip::ZipArchive`, [`ZipFile::enclosed_name() -> Option<PathBuf>`](https://docs.rs/zip/latest/zip/read/struct.ZipFile.html#method.enclosed_name), and `unix_mode`; the existing reference adapter identifies the additional bounds/collision/link requirements. `enclosed_name` alone does not enforce those requirements. Enable only required ZIP codecs; no custom ZIP parser.
- Verification: normal extraction, selected folders, empty directories, malformed/truncated ZIP, traversal, duplicates, case aliases, links, decompression limits, and unchanged destination on failure.

**U5 - Generic repository acquisition/export (`utils/source/git.rs`).**
- Purpose/portability: fetch and export exact repository content into a caller-provided directory, usable by a documentation builder or source archiver. This is the intended content of the existing placeholder.
- Contract: `fetch_repository(repo: &Path, origin: &str, timeout: Duration) -> Result<(), GitError>`, `resolve_commit(repo: &Path, revision: &str) -> Result<CommitId, GitError>`, and `export_commit(repo: &Path, commit: &CommitId, destination: &Path, options: ExportOptions) -> Result<ExportSummary, GitError>`. Git-specific values are acceptable; no Source/App/Claim/Policy model parameters. Export options carry selection and bounds, and the result records exact dependency commits/digests.
- Effects/failures: fetch changes only the supplied repository, validates recorded/effective origin, uses literal arguments and controlled environment, and exports exact committed content including required submodule/LFS objects. Missing tools/auth/content/timeouts fail explicitly; do not run repository hooks. Safe committed links resolve within the complete tree before selection with cycle/bounds checks; unsupported links fail without publication. Caller controls source cache TTL and chooses the ref.
- Reuse: [Git CLI `-C` and explicit arguments](https://git-scm.com/docs/git), standard `Command`, and U6; retain Git/git-lfs as native acquisition owners. [git2 `Repository::open`/`find_commit`](https://docs.rs/git2/latest/git2/struct.Repository.html) were considered: they provide repository access but would introduce native libgit2 and additional LFS integration instead of replacing this full contract. Adapt only generic mechanics from reference Git/export code, not its models, paths, source identities, or policy rules.
- Verification: temporary repositories with spaced paths, refs/pins, origin mismatch, invalid ref arguments, empty trees, safe/broken/cyclic links, dependency failures, and real timeout/nonzero exits. Fixtures are local; GitHub release acquisition never calls this helper.

**U6 - Bounded process execution (`utils/process.rs`).**
- Purpose/portability: execute a supplied program with literal arguments, bounded capture, and timeout; useful for a compiler driver or executable health probe.
- Contract: `run_bounded(command: Command, timeout: Duration, capture_limit: usize) -> Result<ProcessResult, ProcessError>`. Results distinguish exit status, timeout, IO errors, and truncated diagnostics. Do not pass shell strings. Long-running shim launches use standard spawn/wait with inherited IO and domain lease orchestration, not the bounded probe API.
- Effects/failures: drain stdout/stderr concurrently within bounds, kill and reap timed-out children, clean handles, and specify process-tree behavior per platform. No arbitrary executable selection, expected-version interpretation, or app activation in the helper.
- Reuse: [`std::process::Command`](https://doc.rust-lang.org/std/process/struct.Command.html); [`wait_timeout::ChildExt::wait_timeout(Duration)`](https://docs.rs/wait-timeout/latest/wait_timeout/trait.ChildExt.html) supplies waiting but not pipe draining or process-tree cleanup and has Unix signal integration considerations. Prefer standard try_wait/deadline with bounded pipe readers plus platform process-group/job cleanup, rather than introducing that dependency merely to wait. Hidden Windows helpers must not create visible windows.
- Verification: literal arguments, both pipes exceeding capture limits without deadlock, nonzero exits, timeouts, child-tree cleanup, reap behavior, and executable paths containing spaces. Use fixture child binaries, not mocked call counts.

**Direct library use and rejected utility candidates.** Call Serde/serde_json for typed JSON and a UUID crate for IDs; no parsing wrappers. Use a configured [ureq 3 Agent](https://docs.rs/ureq/latest/ureq/) directly in `core/source` for bounded HTTP calls; GitHub response mapping and credential/endpoint selection remain there. Use [`globset::Glob::new`/`compile_matcher`](https://docs.rs/globset/latest/globset/struct.Glob.html) for asset patterns and preserve first-match order in `core/policy`. Use standard time types with caller-provided time for TTL tests. SHA digest computation can call the selected digest library directly. Do not extract source freshness, item fingerprinting, policy precedence, index journals, claims, or useable_app into generic utils: their contracts depend on Saucepan semantics. Current utility dependencies do not justify a graph package or general plugin framework.

**Integration order.** U1/U2/U3 land with domain-neutral tests before persistence/output callers. U4 lands before remote/archive integration. U6 lands before Git and probe callers; U5 follows U6 before repository provider integration. Core code supplies roots, identities, platform choices, policies, and locks; it consumes utility results into source/output records. Core/index owns recovery and claims; core/store owns acquisition/reclaim; core/policy owns eligibility; activation/launch owns binding and running-process lifetime. No successful verification is claimed by this plan.

### 9. `mock/` is the inspectable simulated `.saucepan`

Use `C:/Users/ZackaryWang/Documents/GitHub/saucepan-2/mock` itself as the root, not `mock/.saucepan`. Add an explicit absolute `--root` argument to every CLI command and propagate that context through shim launches and child Saucepan processes; never fall back to the user's production root when an explicit root was supplied. Temporary utility/unit-test roots remain disposable and independent; the final acceptance run leaves `mock/` inspectable. An existing mock root is inspected and reused if compatible, not recursively erased as a convenience. Plan generated runtime contents for gitignore during implementation; keep reproducible fixture recipes/harness outside the runtime root.

```text
mock/
  source/<source-uuid>.json         # github_release metadata
  index/<app-uuid>/index.json       # Saucepan claim and active binding
  index/<entry-uuid>/index.json     # acquired built release provenance
  store/<entry-uuid>/saucepan.exe   # actual built application on Windows
  bin/saucepan.exe                 # built shim, renamed for invocation
```

After implementation: build the actual application and shim in release mode; package the built application as a platform release asset; serve deterministic GitHub-compatible release metadata and assets from a loopback fixture endpoint; acquire via the real `github_release` provider into mock; register the application and useable_app policy; activate; then launch `mock/bin/saucepan.exe --root <absolute-mock-root> ...`. Fixture endpoint injection is explicit test configuration, never a changed production GitHub default. This exercises the download/index/claim/store path rather than fabricating index JSON or manually copying a binary and declaring success. The downloaded executable's digest must equal the packaged local build, and application operations through the shim must resolve that store target while retaining the mock root.

Use two distinguishable builds of the actual package for an actual version-change assertion; identical bytes under two release tags only prove pointer switching and must be reported that way. Exercise fresh/expired discovery with a controlled clock, repeated acquisitions, a failing candidate, a successful replacement, a running old process, rollback, and latest-ten reclaim including ordinary claimed entries. Small fixture children remain useful for focused lease/probe tests, but do not substitute for the final built-Saucepan run. Record build IDs/digests, entry UUIDs, active bindings, process output, and root-isolation checks in the implementation verification report. No implementation, mock population, build, or release download is performed by this planning revision.

## Risks / Trade-offs

- Multiple small JSON files -> Direct lookup for normal operations; scans for retention and recovery initially, with rebuildable indexes only if measured necessary.
- New plain metadata differs from prior encrypted authority -> Explicitly document the local-user trust boundary; do not claim old token/HMAC guarantees or import secrets.
- Policy exceptions can exceed ten outputs -> Report protected/in-use entries and deferred reclaim; never delete the active usable executable to satisfy a count.
- Crash or concurrent activation/reclaim -> Journaled publication, atomic active binding, coordinated leases, and fault-injection tests.
- Shared item policies can retain more outputs than one requester expects -> Persist policies, compute effective retention deterministically, and expose the result.
- Downloaded archives and Git exports contain unsafe paths -> Validate traversal, link handling, entry collisions, integrity, and extraction bounds before publication; retain applicable safety behavior from the reference without inheriting its schema.

## Migration Plan

Build and verify against isolated new-format roots. Refuse legacy store shapes with an actionable message and leave them unchanged. Users create a fresh root and reacquire resources/register apps under the new contract; no automatic migration is supplied. Production bootstrap or replacement of an existing installation is a later explicit deployment action. Roll back a managed application by activating a retained valid output. Returning to the old implementation requires its untouched old root, not decoding the new store with old software.
