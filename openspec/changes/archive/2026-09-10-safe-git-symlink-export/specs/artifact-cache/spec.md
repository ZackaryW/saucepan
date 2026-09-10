## MODIFIED Requirements

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
