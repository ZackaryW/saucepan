# provider-credentials Specification

## Purpose

Defines how provider requests obtain, apply, protect and report credentials: GitHub source types borrow the user's `gh` login, and generic Git platform types read a manually assigned token file.

## Requirements

### Requirement: GitHub source types borrow the gh token
Every `github_repo`, `github_release`, `github_raw` and `github_source` provider operation SHALL first request a token for github.com from the installed `gh` CLI. When a token is returned, every request to GitHub for that operation SHALL carry it. When `gh` is absent, not logged in, fails or exceeds its bounded time limit, the operation SHALL proceed anonymously.

#### Scenario: Logged-in gh authenticates release discovery
- **WHEN** `gh` is logged in and a `github_release` source is discovered
- **THEN** the discovery request carries the borrowed token

#### Scenario: gh is not installed
- **WHEN** `gh` cannot be executed and a public `github_release` source is retrieved
- **THEN** retrieval proceeds without a token and succeeds

#### Scenario: gh hangs
- **WHEN** the `gh` token request does not complete within its time limit
- **THEN** the `gh` process is terminated and the operation proceeds anonymously

### Requirement: Git platform types read a configured token file
`git_repo` and `git_raw` sources MAY name a `token_file`. A leading `~` SHALL expand to the user's home directory. When configured, provider requests SHALL use the file's trimmed contents as the token. A configured file that is missing, unreadable or empty SHALL fail the operation with `source_error` before any provider request. Without `token_file`, requests SHALL be anonymous.

#### Scenario: Token file authenticates a GitLab raw file
- **WHEN** a `git_raw` source on a GitLab host names a readable token file
- **THEN** the raw-file request carries that token in the GitLab token header

#### Scenario: Configured token file is missing
- **WHEN** a `git_repo` source names a token file that does not exist
- **THEN** retrieval fails with `source_error` naming the token file path and contacts no provider

#### Scenario: Home-relative token file
- **WHEN** a source names `~/.config/saucepan/gitlab.token`
- **THEN** the path resolves beneath the current user's home directory

### Requirement: git_repo never consults gh
`git_repo` SHALL authenticate only through its configured `token_file`, including when its origin is a GitHub host. It SHALL NOT invoke `gh`.

#### Scenario: git_repo on github.com without a token file
- **WHEN** a `git_repo` source with a github.com origin and no `token_file` is retrieved while `gh` is logged in
- **THEN** the clone is anonymous and `gh` is not invoked

### Requirement: Tokens are never persisted or disclosed
Borrowed and file-supplied tokens SHALL be held in memory only for the operation that uses them. They SHALL NOT appear in source records, retrieval records, journals, results, logs, error messages, error details or command-line arguments of child processes. Records SHALL store only the `token_file` path. Tokens SHALL NOT be forwarded to a different host on redirect.

#### Scenario: Asset download redirects to another host
- **WHEN** an authenticated GitHub asset request is redirected to a download host
- **THEN** the redirected request carries no `Authorization` header

#### Scenario: Authenticated clone process arguments
- **WHEN** an authenticated `git_repo` or `github_repo` clone runs
- **THEN** the token does not appear in the Git child process arguments

#### Scenario: Provider failure message
- **WHEN** an authenticated request fails
- **THEN** the reported error contains no token text

### Requirement: Provider authorization failures are reported clearly
An HTTP 401 or 403 response from a provider SHALL fail the operation with `source_error`. Its details SHALL include the provider, the HTTP status and whether a token was sent. Its message SHALL say how to authenticate: `gh auth login` for `github_*` types, or checking the configured `token_file` for `git_*` types. A 404 from GitHub when no token was sent SHALL carry the same `gh auth login` guidance.

#### Scenario: Private release without gh
- **WHEN** a `github_release` request for a private repository is sent anonymously and GitHub responds 404
- **THEN** retrieval fails with `source_error` whose message suggests `gh auth login`

#### Scenario: Token lacks access
- **WHEN** an authenticated `git_raw` request receives 403
- **THEN** retrieval fails with `source_error` reporting status 403, that a token was sent, and suggesting checking the token file

#### Scenario: Prior outputs survive a denied refresh
- **WHEN** a due update receives 401 for a source with an existing selected output
- **THEN** the prior output and selection remain unchanged
