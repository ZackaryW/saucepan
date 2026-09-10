## Purpose

Reuse source data centrally at the OS user's level while preserving canonical source identity and checking recorded Git origins on every use.

## ADDED Requirements

### Requirement: Central user-level source reuse
Saucepan SHALL place managed Git, URL, and local source data under the user's `.saucepan` store using SHA-256 canonical source identifiers. A Git identifier SHALL include repository origin and requested branch/ref. Different refs SHALL have different source identifiers and independent current/history records; different folders at the same repo/ref SHALL share one identifier. A branch's resolved commit SHALL be recorded separately so advancing that branch does not change its source identifier. Equivalent locators and the same ref SHALL reuse the central source. Reusing underlying Git objects across refs SHALL NOT merge their source identifiers or current/history records. SHA-256 SHALL also identify acquired content, including URL/local content, while preserving its source provenance. Local input paths SHALL remain user-owned inputs; acquisition SHALL create centrally managed content without modifying those originals. The single shared Saucepan executable SHALL reside in `.saucepan/bin`; acquired artifacts SHALL have separate storage.

#### Scenario: Repeated source in different consumers
- **WHEN** two recipes or consumers acquire different directories from the same canonical Git repository and ref
- **THEN** they reuse one central repository and its source snapshots, extracting the requested folders without creating separate source units or folder-specific current/history records

#### Scenario: Different Git refs
- **WHEN** requests use main and develop from the same repository
- **THEN** their SHA-256 source identifiers differ and updating either ref does not replace the other's current or consume its history slots

#### Scenario: Branch advances
- **WHEN** main advances to a new commit
- **THEN** its source identifier remains stable while its recorded resolved commit and source snapshots advance

#### Scenario: Shared executable placement
- **WHEN** a consumer resolves the shared Saucepan executable
- **THEN** its canonical location is the user-level `.saucepan/bin`, with no requirement for per-app runtime versions or activation machinery

#### Scenario: Reuse URL or local source
- **WHEN** consumers acquire the same canonical URL source or the same canonical local source
- **THEN** they share its central source record and managed content without creating per-consumer source stores

### Requirement: Git origin is recorded and verified every time
The encrypted source index SHALL record the expected Git origin. Each source use, including retained content reuse, SHALL compare the requested identity and configured/effective Git origin with that record. Ambiguous or repointed origins SHALL fail without silently updating the expected origin. Canonical stored encodings SHALL preserve their identity when reopened.

#### Scenario: Repointed repository with cached content
- **WHEN** a repository's configured or effective origin differs from the encrypted record
- **THEN** the operation fails even if a suitable ZIP already exists and optional content verification is off

#### Scenario: Canonical identity reopened
- **WHEN** a GitHub or SSH source is reopened from its recorded canonical identity
- **THEN** it retains the same identity instead of being reinterpreted as a different transport locator

### Requirement: Live content survives cache eviction
Source data required by current or explicitly referenced artifacts SHALL remain available independently of historical ZIP retention. Eviction SHALL NOT delete shared repositories or unrelated consumers' content. Concurrent operations SHALL preserve consistent selected index state or fail without replacing it with mixed state.

#### Scenario: ZIP evicted while content remains referenced
- **WHEN** a historical ZIP is evicted but an artifact still has a live reference
- **THEN** the referenced content remains usable and required source objects are retained

#### Scenario: Concurrent acquisition
- **WHEN** acquisitions compete to update the same current/history state
- **THEN** each successful result corresponds to a consistent publication and a failed operation preserves the committed state

### Requirement: Logical identities are independent of host formatting
Equivalent canonical remote-source/ref inputs and identical logical content SHALL produce the same SHA-256 identifiers on Windows, macOS, and Linux. OS path separators, home-directory placement, and display formatting SHALL NOT change those identifiers. Local source identity SHALL retain the actual local path semantics rather than imply equivalence between unrelated machines' paths. Persisted index data SHALL identify its format version; unsupported versions SHALL fail explicitly without being reinterpreted.

#### Scenario: Same Git source on different platforms
- **WHEN** supported platforms canonicalize the same Git origin and ref
- **THEN** they produce the same source identifier independently of their local store locations

#### Scenario: Unsupported persisted format
- **WHEN** a store contains an unsupported index format version
- **THEN** opening reports incompatibility without replacing or silently converting the recorded state
