## Purpose

Define dependable direct-GitHub target installation, revision selection, authentication, and repository boundaries.

## Requirements

### Requirement: Direct GitHub target normalization
Saucepan SHALL accept a strict `owner/repo` GitHub target with either supported Git backend. When raw `git` is selected, Saucepan SHALL translate only that strict slug form into a GitHub HTTPS clone URL and SHALL pass explicit URLs and filesystem paths through unchanged. Equivalent GitHub spellings SHALL compare as the same repository for central-index matching while preserving the user's original spelling as provenance.

#### Scenario: Raw Git receives a repository slug
- **WHEN** a user installs `owner/repo` with the raw `git` backend
- **THEN** Saucepan clones `https://github.com/owner/repo.git` while retaining `owner/repo` as repository provenance

#### Scenario: Explicit target passes through
- **WHEN** a user installs an explicit URL or filesystem path
- **THEN** Saucepan passes that target to the selected clone backend without GitHub-slug rewriting

#### Scenario: Equivalent target matches an index entry
- **WHEN** an index entry and install target use equivalent GitHub spellings or differ only by a trailing separator or `.git` suffix
- **THEN** Saucepan treats them as the same target without changing the stored provenance spelling

### Requirement: Optional GitHub revision selection
Saucepan SHALL accept an optional `--ref` for GitHub installation, SHALL resolve it to a commit, and SHALL store both the requested ref and resolved commit as optional GitHub index metadata. Omitting `--ref` SHALL preserve default-branch installation and update behavior.

#### Scenario: Install a requested branch
- **WHEN** a user installs a GitHub target with `--ref feature`
- **THEN** Saucepan checks out the commit resolved from the remote `feature` branch and records `feature` plus the resolved commit

#### Scenario: Install a requested tag
- **WHEN** a user installs a GitHub target with `--ref v1.2.0`
- **THEN** Saucepan checks out the commit resolved by that tag and records the requested tag and resolved commit

#### Scenario: Install a commit
- **WHEN** a user installs a GitHub target with `--ref <commit-sha>`
- **THEN** Saucepan checks out that commit and records the requested and resolved revisions

#### Scenario: Install without a ref
- **WHEN** a user installs a GitHub target without `--ref`
- **THEN** Saucepan follows the repository default branch and records the resolved commit without recording a requested ref

#### Scenario: Update a ref-selected installation
- **WHEN** a user updates a GitHub sauce with a stored requested ref
- **THEN** Saucepan resolves the same ref again, checks out its current commit, and refreshes resolved revision and manifest metadata

### Requirement: Backward-compatible revision metadata
Saucepan MUST load GitHub index entries that predate revision metadata. Revision fields SHALL be omitted from serialization when absent.

#### Scenario: Load an existing index
- **WHEN** Saucepan reads a GitHub entry containing only `repo` and `sauce`
- **THEN** it treats the entry as a default-branch installation and performs existing lookup operations successfully

### Requirement: Native private-repository authentication
Saucepan SHALL delegate raw Git authentication to native Git configuration and SHALL pass a configured token to `gh` through `GITHUB_TOKEN`. A token configured with raw Git SHALL produce a warning and SHALL NOT be injected through ineffective username or password environment variables.

#### Scenario: Raw Git with configured token
- **WHEN** an operation uses `binary = "git"` and the source config contains `token`
- **THEN** Saucepan warns once for that operation and continues using the user's native Git credentials

#### Scenario: GitHub CLI with configured token
- **WHEN** an operation uses `binary = "gh"` and the source config contains `token`
- **THEN** Saucepan supplies the token through `GITHUB_TOKEN` to the GitHub CLI authentication path

### Requirement: Repository-root manifest boundary
Saucepan SHALL read the configured manifest filename from the cloned repository root. It SHALL NOT interpret target subdirectories or GitHub Release assets as installable GitHub targets.

#### Scenario: Manifest exists only in a subdirectory
- **WHEN** a cloned repository lacks the configured manifest at its root
- **THEN** Saucepan reports that the target cannot supply a sauce even if a file with that name exists in a nested directory
