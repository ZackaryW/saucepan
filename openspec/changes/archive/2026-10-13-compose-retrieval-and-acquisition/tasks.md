# Tasks

Each numbered group is a coherent checkpoint with its own tests and documentation. Inspect its diff and run relevant checks before dependent work; commit explicit paths when authorized using the applicable commit guidance. Active planning artifacts remain outside implementation checkpoints unless separately authorized. Do not run hosted CI or alter historical mock/user stores for these tasks.

## 1. Typed composition contracts

- [x] 1.1 Add retrieval models and acquisition destination/update/revision models with explicit ownership and provenance; verify serialization and validation tests for the supported composition matrix, contradictory options and missing destinations.
- [x] 1.2 Define guarded native result interfaces and translate existing convenience eviction settings into one revision policy; verify existing request fixtures retain defaults while new requests reject mixed spellings and unsupported managed effects.
- [x] 1.3 Document the retrieval/acquisition/update/revision vocabulary and supported combinations alongside the API; verify examples against DTO parsing tests and compile the public interfaces.

## 2. Reusable transfer and offline materialization utilities

- [x] 2.1 Extract bounded transfer with optional digest calculation using existing dependencies; verify byte limits, short/error reads and writes, digest vectors, and failure cleanup with focused tests.
- [x] 2.2 Split Git dependency hydration from offline export using explicit repository, dependency and scratch paths; verify exact commits, submodules, LFS and zero provider fetching during offline export.
- [x] 2.3 Adapt copy/extraction delivery for optional hashing and coordinated create-new destinations; verify containment, executable metadata, occupied/racing destinations, disjoint paths and rollback without replacing caller files.
- [x] 2.4 Document utility contracts and caller coordination requirements; verify utility tests and relevant lint checks pass without introducing source/cache policy into utils.

## 3. Fresh retrieval without managed storage

- [x] 3.1 Implement inline temporary and no_store retrieval for Git, GitHub releases, URL and local sources; verify fresh origin requests despite prepopulated caches and no registration, home lookup, cache reads/writes or keyring access.
- [x] 3.2 Return original material representations, pinned Git provenance and complete dependencies with owned lifetimes; verify archive bytes remain unextracted and captured local content remains stable after the input changes.
- [x] 3.3 Implement none/if_available/required expected-digest verification and truthful evidence; verify mismatch/missing-digest failures, contradictory inputs, optional hashing and mandatory structural checks.
- [x] 3.4 Document fresh retrieval examples and ownership; verify native examples on disposable paths including provider failure, destination collision and cleanup after the last dependent handle.

## 4. Managed source retrieval and independent cache lifecycle

- [x] 4.1 Add typed retrieval records, catalog membership and journaled publication under source-owned material paths; verify recovery and metadata integrity in basic mode and existing isolated keyring test infrastructure where available.
- [x] 4.2 Implement cached freshness modes, source-default configuration and per-call overrides; verify 24-hour boundaries, cache-only missing dependencies, stale fallback timestamps and concurrent configuration/refresh without lost updates.
- [x] 4.3 Add guarded repository/material access and explicit retrieval purge with bounded coordination; verify live readers defer mutation/deletion, stronger verification rechecks weak receipts, and purge preserves acquired outputs, source registration and shared Git objects.
- [x] 4.4 Document source cache layout, representations and cleanup independently of output history; verify cached retrieval and purge examples against a disposable root without creating output records.

## 5. Acquisition from retrieved material

- [x] 5.1 Implement offline copying, Git subpath export and ZIP extraction from retrieved handles to managed, temporary and caller-owned destinations; verify independent bytes, no mutable aliases and no network when no update is due.
- [x] 5.2 Wire existing managed acquisition through the new composition while preserving publication identity, verification and result compatibility; verify existing acquisition/reuse/new_entry and publication-recovery tests.
- [x] 5.3 Enforce destination and lifetime contracts before effects; verify fresh retrieval cannot silently create managed outputs/claims and cached retrieval can supply caller-owned output without managed history.
- [x] 5.4 Document both path APIs with a one-repository/two-subpaths example; verify it performs one retrieval and returns a repository path plus two independent copied paths.

## 6. Acquisition update and revision policies

- [x] 6.1 Add independent managed acquisition check state and per-intent coordination; verify separate scopes, unclaimed intent deduplication, policy changes and journal recovery without mutating immutable output records.
- [x] 6.2 Evaluate manual/always/max_age update before reuse and orchestrate fresh retrieval when due; verify fresh source TTL cannot hide a changed branch, pinned commits stay pinned, satisfied checks avoid duplicate fetches and contradictory cache-only requests fail before effects.
- [x] 6.3 Compare selected content and preserve separate checked/selected provenance; verify unrelated Git changes reuse a compatible output, selected/dependency changes publish a new revision and explicit new_entry still allocates one.
- [x] 6.4 Preserve previous selection and successful-check time on failures or stale fallback; verify failed retrieval, verification, materialization and publication do not report a successful update or alter active bindings.
- [x] 6.5 Apply history none/latest through the single revision evaluator; verify default ten, keep-one behavior, unchanged reuse, maximum surviving policy demand, deferred live/active revisions and source-cache independence.
- [x] 6.6 Document demand-driven updates and revision history with examples; verify the examples demonstrate independent fetch frequency, copied-result updates and history retention.

## 7. Optional claims with explicit policy enforcement

- [x] 7.1 Validate app source/action/scope restrictions for retrieval and acquisition whether or not a claim is requested; verify denied fresh/unclaimed requests cannot become administrative requests and anonymous operations create no synthetic app.
- [x] 7.2 Integrate due updates into claim ensure using the existing claim identity and registered update/revision allowances; verify concurrent ensures, policy conflicts, scope independence and existing context/release/uninstall revision fences.
- [x] 7.3 Add the registered fresh-operation metadata-read path without implicit recovery mutation; verify pending recovery gives a prerequisite error, basic mode needs no keyring and successful requests leave cache/catalog state unchanged.
- [x] 7.4 Document optional credentials, optional claims and mandatory configured restrictions separately; verify examples cover scoped claims and app-authorized operations without claims.

## 8. CLI paths and lifetime-bearing consumers

- [x] 8.1 Add retrieve, acquire-retrieved, source-path, configure-source and purge-retrieval commands with typed errors and root resolution only when required; verify help, missing-home fresh operations, structured results and rejection of untrusted managed descriptors.
- [x] 8.2 Add run-retrieved and run-acquired using existing native child lifetime mechanics; verify literal arguments, streams, exit codes, representation/environment values, failed launch cleanup and consumer survival after abrupt wrapper exit on each executed host.
- [x] 8.3 Reject temporary bare-path delivery and coordinate refresh without upgrading a held source lock; verify useful errors and a due update while retaining the prior output without deadlock or premature cleanup.
- [x] 8.4 Update README and CLI/API examples to distinguish cached, fresh, temporary, caller-owned and claimed use; run the documented examples against disposable roots and confirm ordinary acquire/query/lifecycle commands remain compatible.

## 9. Cross-capability acceptance

- [x] 9.1 Exercise a local Git integration fixture through cached retrieval, two subpaths, branch updates inside the TTL, unchanged-folder reuse, changed-folder publication and no-history deferred reclaim; verify exact provenance and request counts through the full flow.
- [x] 9.2 Exercise ZIP and Git fresh-mode consumer flows with preexisting caches and unavailable home/keyring; verify no cache reads or writes, correct source versus acquired paths and native lifetime cleanup.
- [x] 9.3 Run formatting, warning-denying lint and the relevant acquisition, claims, query, recovery, lifecycle and shim regression suite for the final integrated diff; report executed hosts and unavailable checks honestly without triggering paid multi-platform workflows.
- [x] 9.4 Reconcile proposal/design/specs, public docs and completed tasks against the actual implementation; run strict OpenSpec validation and review the final diff for unrelated changes, unsupported compatibility promises or Power Eagle boundary crossings.
