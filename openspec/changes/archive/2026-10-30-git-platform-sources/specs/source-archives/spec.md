# Spec Delta

## Purpose

Lets callers obtain a GitHub repository's files at one commit as a source snapshot ZIP through `github_source`, without cloning history, and acquire all or a folder of it.

## ADDED Requirements

### Requirement: github_source retrieves a snapshot at a resolved commit
A `github_source { repo }` source SHALL accept `ref`, `tag` and `commit` selectors. Retrieval SHALL resolve the selector to an exact commit through the GitHub API and download the repository ZIP archive for that commit through the GitHub API. The result SHALL be ZIP file material with provenance identifying the exact commit. No Git repository data SHALL be created.

#### Scenario: Snapshot of a branch
- **WHEN** a `github_source` source is retrieved with ref `main`
- **THEN** retrieval records the resolved commit and returns the archive for that commit
- **AND** no bare repository is created for the source

### Requirement: Snapshot extraction removes the archive root folder
Acquisition of `github_source` material SHALL require the archive to contain exactly one top-level directory and SHALL extract its contents as the repository root. Folder selection SHALL be relative to the repository root, not the archive root folder.

#### Scenario: Folder selection without the generated prefix
- **WHEN** a `github_source` snapshot is acquired with folder `skills`
- **THEN** the output contains the repository's `skills` folder contents regardless of the archive root folder name

#### Scenario: Unexpected archive layout
- **WHEN** the downloaded archive has more than one top-level entry
- **THEN** acquisition fails without publishing an output

### Requirement: Snapshot limits fail or are reported explicitly
`github_source` acquisition SHALL apply the existing archive safety rules, including rejection of link entries, and SHALL fail with a message identifying a link entry when one is present. Submodule content and Git LFS objects are not part of snapshot material; this SHALL be documented, and LFS pointer files SHALL be delivered unchanged.

#### Scenario: Repository contains a symbolic link
- **WHEN** a snapshot archive contains a symbolic link entry
- **THEN** acquisition fails and the message identifies the link entry

#### Scenario: LFS pointer in snapshot
- **WHEN** a snapshot contains a Git LFS pointer file
- **THEN** the output contains the pointer file text, not the LFS object
