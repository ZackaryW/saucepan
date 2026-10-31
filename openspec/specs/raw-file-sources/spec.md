# raw-file-sources Specification

## Purpose

Lets callers retrieve one file from a hosted Git repository at a ref, tag or commit without cloning, on GitHub through `github_raw` and on GitLab or Gitea through `git_raw`.

## Requirements

### Requirement: github_raw retrieves one file at a resolved commit
A `github_raw { repo, path }` source SHALL accept `ref`, `tag` and `commit` selectors. Retrieval SHALL resolve the selector to an exact commit through the GitHub API, then download `path` at that commit from GitHub's raw-file endpoint. The result SHALL be file material with provenance identifying the exact commit.

#### Scenario: Branch file is retrieved
- **WHEN** a `github_raw` source for `owner/repo` and `skills/a/SKILL.md` is retrieved with ref `main`
- **THEN** retrieval records the commit `main` resolved to and returns that file's bytes at that commit

#### Scenario: Exact commit is pinned
- **WHEN** a `commit` selector names a full commit ID
- **THEN** the file is downloaded at that commit and the recorded commit equals it

### Requirement: git_raw retrieves one file through its platform URL
A `git_raw { host, platform, repo, path }` source SHALL accept `platform` values `gitlab` and `gitea` and `ref`, `tag` and `commit` selectors. Retrieval SHALL download the file from that platform's raw-file URL for the selector and SHALL record the exact requested URL with any returned validators as provenance. It SHALL NOT resolve refs to commits.

#### Scenario: GitLab raw file
- **WHEN** a `git_raw` source with platform `gitlab` is retrieved for ref `main`
- **THEN** the file is downloaded from the host's GitLab raw route for `main` and its URL is recorded as provenance

#### Scenario: Gitea raw file
- **WHEN** a `git_raw` source with platform `gitea` is retrieved for tag `v1`
- **THEN** the file is downloaded from the host's Gitea raw route for `v1`

#### Scenario: Unsupported platform
- **WHEN** a `git_raw` source names platform `bitbucket`
- **THEN** registration fails with `invalid_request` before any network request

### Requirement: Raw sources validate their inputs before network access
`repo` and `path` SHALL be validated before any request. `path` SHALL be a contained relative file path without empty, `.` or `..` segments. `host` SHALL be an HTTPS host name without path, query, fragment or credentials. Invalid inputs SHALL fail with `invalid_request`.

#### Scenario: Escaping path
- **WHEN** a raw source names path `../secrets`
- **THEN** registration fails with `invalid_request`

#### Scenario: Host contains credentials
- **WHEN** a `git_raw` host is given as `user:pass@gitlab.example.com`
- **THEN** registration fails with `invalid_request`

### Requirement: Raw file material follows the file output contract
Raw sources SHALL produce file material that acquisition copies as a single file in natural format, under the existing filename rules. Raw sources SHALL NOT accept folder selection or ZIP extraction.

#### Scenario: Raw file acquisition
- **WHEN** a `github_raw` source for `docs/guide.md` is acquired
- **THEN** the output contains `guide.md` with the downloaded bytes

#### Scenario: Folder selection rejected
- **WHEN** a raw source acquisition requests a folder selection
- **THEN** it fails with `invalid_request` before network access
