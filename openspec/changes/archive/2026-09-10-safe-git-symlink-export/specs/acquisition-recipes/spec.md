## MODIFIED Requirements

### Requirement: Export complete selected Git content
A Git recipe SHALL select a normalized directory from the complete exported source snapshot, with that directory's complete contents at the extraction output root. The retained ZIP SHALL contain the complete exported source tree; directory selection SHALL NOT create a separate cached ZIP or current/history queue. Included submodules SHALL use recorded gitlink commits and included LFS objects SHALL contain validated actual bytes. Safe committed Git symlinks SHALL resolve into independent ordinary files or directories before folder selection. Resolution SHALL use only the complete committed source tree, including its resolved submodules, and SHALL NOT read host paths. Missing required content, unsafe links, and invalid selections SHALL fail before publication. Arbitrary repository-defined installer/filter programs SHALL NOT be used to complete an artifact.

#### Scenario: Selected subdirectory
- **WHEN** a recipe selects a directory inside a Git repository, including across a submodule boundary
- **THEN** the extracted output contains that directory's complete exported contents and excludes siblings and wrapper directories, while the shared source snapshot retains the other source folders

#### Scenario: Missing required data
- **WHEN** an included submodule commit or LFS object cannot be resolved or validated
- **THEN** acquisition fails without publishing a partial artifact

#### Scenario: Unsafe selection or link
- **WHEN** a selection escapes the source snapshot or a link resolves outside the complete committed source tree
- **THEN** Saucepan rejects it before publishing content

#### Scenario: Link to a sibling outside the selected folder
- **WHEN** `sdk/config` links to `../shared/config`, both paths exist in the committed source, and the caller selects `sdk`
- **THEN** output contains the resolved ordinary content at `config` without exposing a filesystem link or separately extracting the entire `shared` directory

#### Scenario: Selected directory is a safe alias
- **WHEN** the selected folder is a committed symlink to a directory inside the source tree
- **THEN** its resolved directory contents appear at the extraction root

## ADDED Requirements

### Requirement: Resolve Git aliases with portable relative-path semantics
Relative link targets SHALL resolve from the committed link's parent in the complete source tree. Intermediate directory aliases and link chains SHALL resolve before subsequent path components are interpreted. Internal `.` and `..` components SHALL be supported while traversal above the source root SHALL fail. Resolved files SHALL contain the final target's bytes and executable metadata; directory aliases SHALL contain independent copies of their resolved descendants. No native symlink or hardlink SHALL be created. These rules SHALL apply on Windows, Linux, and macOS without elevation or symlink privileges.

#### Scenario: File chain and executable target
- **WHEN** a relative link chain ends at a committed executable file
- **THEN** every exported alias contains that file's bytes and recorded executable metadata as an ordinary file

#### Scenario: Directory aliases are independent
- **WHEN** two aliases refer to the same committed directory
- **THEN** both export successfully as independent directory trees and are not mistaken for a cycle merely because they share a target

#### Scenario: Included dependency targets
- **WHEN** a link resolves into or out of an included submodule within the complete source tree, or resolves to an LFS-backed file
- **THEN** export uses the recorded submodule commit and validated LFS bytes, retaining their dependency evidence

### Requirement: Reject unsafe or excessive Git link expansion before publication
Absolute targets, Windows drive/UNC forms, backslash-containing targets, empty or invalid UTF-8 targets, NUL-containing targets, missing targets, traversal through files, and access to excluded Git administration paths SHALL fail explicitly. Link cycles, recursive directory expansion, and resource-limit exhaustion SHALL fail without publishing new current/history pointers or app touches. The checks SHALL remain mandatory with snapshots or repeated content verification disabled. Link/export failures SHALL NOT qualify for remote-unavailability cache fallback.

#### Scenario: Cycle or broken target
- **WHEN** a committed link is broken, links eventually refer back to one another, or a directory alias recursively includes itself
- **THEN** acquisition reports the resolution failure without hanging or publishing partial state

#### Scenario: External or administrative target
- **WHEN** a link would read a host absolute path, escape the source root, or access `.git` metadata through any alias
- **THEN** export fails without reading or including that target

#### Scenario: Expansion limits
- **WHEN** resolving one output path would follow more than 64 link hops, or the complete expanded export would exceed 1,000,000 entries or 16 GiB of file bytes
- **THEN** export fails explicitly before publication, counting duplicated alias content toward the limits

#### Scenario: Unsafe next revision with fallback enabled
- **WHEN** a source has valid current/history, its next commit contains an unsafe link, and the app allows local fallback
- **THEN** acquisition fails, the previous selection and app touches remain unchanged, and the failure is not reported as a successful cached acquisition

#### Scenario: Other providers retain their boundaries
- **WHEN** a local source or downloaded ZIP contains a filesystem or archive symlink
- **THEN** existing link-rejection behavior applies; Git link resolution is not invoked
