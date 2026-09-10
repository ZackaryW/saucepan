# artifact-materialization Specification

## Purpose

Provide usable central artifact directories and optional independent copies without coupling their lifetime to historical ZIP-cache entries.

## Requirements

### Requirement: Central content and explicit mirrors
Saucepan SHALL produce content in its user-level store and SHALL create an independent mirror only when requested. Directory-path results SHALL identify usable extracted content. ZIP eviction SHALL NOT remove live extracted artifacts or mirrors. Acquisition alone SHALL NOT imply a separate named installation registry.

#### Scenario: Central content reuse
- **WHEN** a caller requests an acquired artifact's content directory
- **THEN** Saucepan returns usable centrally stored content for the recorded artifact

#### Scenario: Explicit mirror
- **WHEN** a caller requests a mirror at a valid destination
- **THEN** Saucepan creates an independent copy whose edits cannot change the central artifact or another mirror

#### Scenario: Different folders from one source snapshot
- **WHEN** callers request different folders from the same source revision
- **THEN** Saucepan extracts each requested folder from the shared source snapshot without creating folder-specific snapshot histories or treating the folders as separate source units

### Requirement: Safe publication and removal
Materialization SHALL reject escaping paths, unsafe links, and unrepresentable entries before publishing content. It SHALL preserve unrelated or locally modified destination files instead of overwriting or deleting them silently. Removal SHALL affect only recorded content and references. These checks SHALL apply with optional content verification disabled and SHALL NOT require destination-grant administration or custom OS permissions.

#### Scenario: Unsafe archive entry
- **WHEN** an entry would write outside its intended artifact directory
- **THEN** materialization fails without writing that entry outside the destination

#### Scenario: Occupied or edited destination
- **WHEN** a requested replacement or removal encounters unrelated or locally edited files
- **THEN** Saucepan preserves those files and reports the conflict

#### Scenario: Entries collide on the target filesystem
- **WHEN** distinct archive paths would collide or cannot be represented on the destination filesystem
- **THEN** materialization fails before publication without silently renaming or overwriting those entries
