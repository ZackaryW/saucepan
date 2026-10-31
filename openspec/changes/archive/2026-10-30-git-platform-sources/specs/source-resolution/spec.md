# Spec Delta

## MODIFIED Requirements

### Requirement: GitHub release sources acquire assets without cloning
A `github_release` source SHALL resolve latest stable releases or explicit release tags and expose their asset metadata for selection. It SHALL NOT clone or fetch a Git repository to acquire release assets. Resolved provenance SHALL identify the release and selected asset, not only a mutable latest-download URL. The selected asset SHALL be downloaded through the GitHub release asset API by asset id, using provider credentials as defined by provider-credentials. Recorded provenance SHALL be identical whether or not a token was used.

#### Scenario: Saucepan acquires its own release
- **WHEN** its release source resolves a matching executable archive
- **THEN** acquisition downloads the selected asset and records its release/asset identity
- **AND** no Git checkout is created for that release source

#### Scenario: Private release asset
- **WHEN** `gh` is logged in with access to a private repository and its release asset is selected
- **THEN** the asset downloads through the asset API by id and is recorded with the same release/asset identity as a public asset

### Requirement: Repository sources maintain reusable source data
`git_repo` and `github_repo` retrieval SHALL operate on repository data, resolve requested refs to exact commits and obtain required dependencies before returning. `git_repo` SHALL clone its normalized origin; `github_repo` SHALL clone `https://github.com/<repo>.git`. A `token_file` on a `git_repo` with a non-HTTP(S) origin SHALL fail with `invalid_request`. Cached retrieval SHALL maintain reusable repository data separately from outputs; temporary and no_store SHALL use fresh isolated repository material. Acquisition SHALL export/copy selected subpaths from the retrieved exact commit and SHALL not expose repository data as its copied output. Future fetches SHALL NOT mutate published outputs. Required submodule and LFS contents SHALL be retrieved consistently or retrieval SHALL fail without returning complete material. Copying from an adequate retrieval SHALL require no network; a due acquisition update SHALL explicitly request a new retrieval first.

#### Scenario: A tracked branch advances
- **WHEN** a due refresh resolves a new commit and a selected folder is acquired
- **THEN** the new output records that exact commit and the previous output remains unchanged

#### Scenario: github_repo is persisted as its own type
- **WHEN** a caller registers `github_repo { repo: "Owner/Repo" }`
- **THEN** the stored source has type `github_repo` with its repository name, and is not rewritten into a `git_repo` source

#### Scenario: Token file on an SSH origin
- **WHEN** a `git_repo` source with an `ssh://` origin names a `token_file`
- **THEN** registration fails with `invalid_request`

#### Scenario: The git type is rejected
- **WHEN** a caller registers a source with type `git`
- **THEN** registration fails with `invalid_request` and suggests `git_repo` or `github_repo`
