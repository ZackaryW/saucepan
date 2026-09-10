# artifact-cache Specification

## Purpose

Maintain one current snapshot and up to five historical ZIPs per source identifier using least-recently-used eviction. Git identifiers include the ref. Folder requests at the same repo/ref extract from those shared snapshots; app settings determine whether an operation retains snapshots.

## Requirements

### Requirement: Git snapshots exclude administration data
Every retained Git snapshot SHALL be a ZIP of the complete exported source tree, with paths relative to its repository root and without `.git` directories or gitfiles at any depth. Normal exported files such as `.gitignore` SHALL be preserved. Safe committed symlinks SHALL appear at their alias paths as resolved ordinary file or directory entries; symlink ZIP entries and link-target pointer text SHALL NOT substitute for resolved content. Snapshot identity SHALL account for source identity, revision, export behavior, and included dependencies, with the content manifest describing the resolved bytes and executable metadata. A requested subdirectory SHALL identify content to extract from that snapshot, not a separate snapshot unit.

#### Scenario: Nested Git content
- **WHEN** a source tree includes submodules or Git administration paths
- **THEN** its ZIP contains the required exported content and no `.git` entries

#### Scenario: Two folders at one commit
- **WHEN** callers request app-a/ and app-b/ from the same source at the same commit
- **THEN** both are extracted from the same source snapshot, their outputs contain their respective folder contents, and no second snapshot or folder-specific history is created

#### Scenario: Portable alias snapshot
- **WHEN** a committed source contains safe file and directory links
- **THEN** its ZIP, extracted content, and mirrors contain ordinary resolved content without needing host symlink support

#### Scenario: Reuse and changed target
- **WHEN** an unchanged source revision is reacquired, or a later revision changes a link target or its contents
- **THEN** unchanged inputs reuse the same resolved snapshot while changed inputs follow the existing current/history identity and rolling rules

#### Scenario: Existing link-free snapshot
- **WHEN** a source revision previously acquired without links is reacquired after this change
- **THEN** its snapshot identity and historical readability remain compatible when its resolved content and dependency inputs are unchanged

### Requirement: Current advances through a five-entry historical LRU
Each source identifier SHALL have one current snapshot separate from at most five historical source ZIPs. Git refs SHALL distinguish source identifiers; subdirectories at the same repo/ref SHALL share current and history rather than create separate records or queues. A successful changed acquisition with app retention enabled SHALL preserve an available outgoing current snapshot in history before replacing it. History eviction SHALL use least recent use, with deterministic oldest-entry tie breaking, without a frequency counter or exemption for installed artifacts. Reading history, including extracting a folder from it, SHALL refresh the source snapshot's recency without changing current.

#### Scenario: Full history advances
- **WHEN** current F advances to G with history A-E and A is least recently used
- **THEN** committed state contains current G and history B-F

#### Scenario: Unchanged acquisition
- **WHEN** refreshed source inputs match current, including when a different folder is requested
- **THEN** current is reused without adding a duplicate historical ZIP

#### Scenario: Historical read
- **WHEN** a retained historical source snapshot is read or a folder is extracted from it
- **THEN** its recency is refreshed while the current acquisition remains unchanged

### Requirement: App retention settings govern snapshot creation
Disabling an app's retention setting SHALL stop creation of new retained source ZIPs for its operations while preserving resolved source-current metadata and producing the requested content. It SHALL NOT change another app's settings or remove already retained shared history. Re-enabling retention SHALL start saving from the app's next successful acquisition. Missing history from copies that were never retained SHALL remain missing; Saucepan SHALL NOT backfill, reconstruct, or fail solely to recover them. An existing retained outgoing current SHALL follow the normal rolling-history rule. No history entry SHALL be evicted merely because an unsaved outgoing copy is skipped.

#### Scenario: Retention disabled
- **WHEN** a changed recipe is acquired with retention off
- **THEN** it produces content and records the resolved source current without admitting a new ZIP to current/history storage

#### Scenario: App retention re-enabled
- **WHEN** app A re-enables snapshots
- **THEN** subsequent A operations use retention while other apps keep their own settings, without creating an app-owned history queue

#### Scenario: Missing outgoing snapshot after retention resumes
- **WHEN** F was acquired without retention and never saved, then the app enables retention and acquires G
- **THEN** G is saved as current, F remains absent from history, and acquisition succeeds without reconstructing F or evicting history to represent the gap

#### Scenario: Unchanged content after retention resumes
- **WHEN** the app re-enables retention and its next acquisition resolves to F whose current ZIP was never retained
- **THEN** Saucepan saves F as current without fabricating historical snapshots

### Requirement: Failed transitions preserve committed current and history
Acquisition SHALL prepare content before publishing a changed source current/history. Failed preparation, required outgoing preservation, or publication SHALL retain the last consistent source state. Locally edited materializations or mirrors SHALL NOT become source snapshots.

#### Scenario: Incoming export fails
- **WHEN** G cannot be fully prepared while F is current
- **THEN** F and the previous history remain selected

#### Scenario: Edited mirror
- **WHEN** a caller edits a mirror and later refreshes its recipe
- **THEN** any historical snapshot comes from recorded source inputs, not the edited mirror
