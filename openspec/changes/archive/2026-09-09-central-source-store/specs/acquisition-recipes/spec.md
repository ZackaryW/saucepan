## Purpose

Describe selected content declaratively so acquisition can reuse central sources without coupling recipes to cache policy or application authority.

## ADDED Requirements

### Requirement: Recipes support Git, URL, and local sources
A recipe SHALL identify its provider, source locator, and content-selection inputs appropriate to that provider. Git, URL, and local SHALL all be implemented in this change. Git recipes SHALL support repository URLs, revision and directory selection; URL recipes SHALL additionally support direct ordinary-file and archive downloads; local recipes SHALL acquire content from their local path. All three SHALL use the central source store, SHA-256 identification, and the calling app's independent cache/verification settings. URL and local SHALL NOT be left as stubs, unsupported variants, or future extension points. Supported archive formats SHALL be documented; unsupported formats/providers and executable acquisition instructions SHALL fail explicitly before publishing content.

#### Scenario: Recipe and policy are independent
- **WHEN** the same Git, URL, or local recipe is acquired with different cache or verification policies
- **THEN** it selects the same content inputs without adding snapshot settings to an installed unit

#### Scenario: Unsupported provider
- **WHEN** a recipe requests a provider that is not implemented
- **THEN** Saucepan reports unsupported acquisition without interpreting the request as Git

#### Scenario: URL acquisition
- **WHEN** a caller acquires a URL source whose requested content is available
- **THEN** Saucepan downloads it into the central store, records its URL source identity and resolved content, and returns usable content without treating the URL as a Git remote

#### Scenario: Direct ordinary file and archive downloads
- **WHEN** an app requests an ordinary file URL or an archive URL in a supported format
- **THEN** Saucepan downloads the content, identifies it using SHA-256, and provides the file or safely extracted archive contents as requested without requiring a Git repository

#### Scenario: Local acquisition
- **WHEN** a caller acquires a local source whose requested content is available
- **THEN** Saucepan acquires that content into the central store, records its local source identity and resolved content, and leaves the original source path unchanged without requiring a Git repository

#### Scenario: Missing local source or failed URL download
- **WHEN** required local content is unavailable or a URL download fails without an applicable allowed cache fallback
- **THEN** acquisition fails explicitly without publishing partial content or reporting an unsupported-provider placeholder

### Requirement: Resolve tracked and pinned Git inputs truthfully
Tracked acquisition SHALL attempt to refresh and resolve the requested branch before choosing cached content. If the remote update check fails, the local-cache fallback policy SHALL determine whether a suitable recorded local copy can be returned, as specified in artifact-policies. A commit pin or explicit resolved artifact read SHALL retain its exact revision; fallback SHALL NOT substitute a different revision. Every result SHALL identify its actual source, revision, selection, and included dependency inputs, and SHALL disclose when a failed update check caused local-cache fallback. Source identity, recipe identity, and content identity SHALL remain distinct.

#### Scenario: Tracking with an existing cache
- **WHEN** a tracked branch has advanced beyond cached current
- **THEN** acquisition prepares the newly resolved commit instead of returning old current as latest

#### Scenario: Refresh failure with fallback disallowed
- **WHEN** a tracked acquisition cannot refresh its remote and policy disallows local-cache fallback
- **THEN** it fails without advancing current or reporting stale content as latest

#### Scenario: Refresh failure with fallback allowed
- **WHEN** a tracked acquisition cannot refresh its remote and policy allows fallback to a suitable recorded local copy
- **THEN** it returns that copy's actual revision, reports that updates could not be checked, and leaves source current and history membership unchanged

#### Scenario: Exact artifact read
- **WHEN** an existing resolved artifact is requested
- **THEN** Saucepan uses its recorded inputs without advancing the source's current snapshot

### Requirement: Export complete selected Git content
A Git recipe SHALL select a normalized committed directory to extract from the shared source snapshot, with that directory's complete contents at the extraction output root. The retained ZIP SHALL contain the complete exported source tree; directory selection SHALL NOT create a separate cached ZIP or current/history queue. Included submodules SHALL use recorded gitlink commits and included LFS objects SHALL contain validated actual bytes. Missing required content, unsafe links, and invalid selections SHALL fail before publication. Arbitrary repository-defined installer/filter programs SHALL NOT be used to complete an artifact.

#### Scenario: Selected subdirectory
- **WHEN** a recipe selects a directory inside a Git repository, including across a submodule boundary
- **THEN** the extracted output contains that directory's complete exported contents and excludes siblings and wrapper directories, while the shared source snapshot retains the other source folders

#### Scenario: Missing required data
- **WHEN** an included submodule commit or LFS object cannot be resolved or validated
- **THEN** acquisition fails without publishing a partial artifact

#### Scenario: Unsafe selection or link
- **WHEN** the selection or exported link would escape the selected artifact
- **THEN** Saucepan rejects it before publishing content
