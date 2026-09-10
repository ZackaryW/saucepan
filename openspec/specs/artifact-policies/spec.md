# artifact-policies Specification

## Purpose

Keep caching and optional repeated content verification independent of acquisition recipes and installed artifacts while retaining essential consistency checks.

## Requirements

### Requirement: Policies belong to the calling app
Cache retention, content verification, and whether local-cache fallback is allowed SHALL be independent settings of the registered calling app, persisted in the central encrypted index. Saucepan SHALL read the app's settings and initial filters from that record; it SHALL NOT load app-root TOML settings. Registration and configuration SHALL create or update those central settings. An operation SHALL use that app's settings without changing another app's settings. Installed artifacts SHALL NOT own snapshot queues or mutable cache-policy definitions. Default retention SHALL be on and optional repeated content verification SHALL be off. Local-cache fallback SHALL be disabled by default; a failed update check SHALL return an error unless the app's stored policy explicitly enables fallback. Acquisition recipes and cache/verification settings SHALL remain separate from the app's index filtering and SHALL NOT disable mandatory checks.

#### Scenario: Apps have different snapshot settings
- **WHEN** app A enables snapshots and app B disables them while using the same source/ref
- **THEN** each operation uses its calling app's setting, B's setting does not delete existing snapshots or change A's setting, and retained history remains attached to the shared source/ref rather than creating per-app queues

#### Scenario: Different verification and fallback settings
- **WHEN** two apps use different verification or local-cache fallback settings
- **THEN** each receives behavior determined by its own configuration even when source content is reused

#### Scenario: Default policy
- **WHEN** a recipe is acquired with default policies
- **THEN** retention is enabled, repeated content verification is disabled, and local-cache fallback is disabled

#### Scenario: Independent controls
- **WHEN** a caller selects cache-off acquisition with content verification enabled
- **THEN** Saucepan produces verified content without adding retained ZIP snapshots

### Requirement: Policy controls local-cache fallback after an update-check failure
When a remote update check fails, Saucepan SHALL use a suitable recorded local copy only if policy allows local-cache fallback. The copy SHALL correspond to the requested source and contain the required content; an explicit revision SHALL still be respected. Results SHALL report the actual cached revision, that local fallback occurred, and that updates could not be checked. Fallback SHALL NOT claim the content is latest, advance source current, or add or evict historical snapshots; normal read-recency updates may occur. With fallback disallowed or no suitable local copy available, acquisition SHALL fail explicitly. Fallback SHALL retain mandatory checks and requested content verification and SHALL NOT recover from an integrity failure by selecting an unchecked path.

#### Scenario: Allowed fallback
- **WHEN** an update check fails, local-cache fallback is allowed, and a suitable recorded local copy passes applicable checks
- **THEN** Saucepan returns that copy with its revision and a clear fallback/update-check-failed result, without rolling source history

#### Scenario: No usable local copy
- **WHEN** an update check fails and fallback is allowed but the requested content is unavailable locally
- **THEN** acquisition fails without returning partial or unrelated cached content

#### Scenario: Fallback disabled
- **WHEN** an update check fails and policy disallows fallback, including the default policy, even though a local copy exists
- **THEN** acquisition fails without returning the local copy as a successful tracked acquisition

#### Scenario: Verification still fails
- **WHEN** fallback is allowed but index, origin, applicable sub-index, or requested content verification fails
- **THEN** the failure remains an error rather than enabling unchecked cache reuse

### Requirement: Optional verification does not disable consistency
Saucepan SHALL record content digests at creation and compare the consumed content with those records when verification is requested. Results SHALL identify whether optional content verification was performed. Index authentication, recorded Git-origin consistency, required LFS object validation, authoritative sub-index verification when applicable, and safe extraction SHALL remain active regardless of the optional setting.

#### Scenario: Fast reuse
- **WHEN** recorded content is reused with optional verification disabled
- **THEN** mandatory checks still apply and the result does not claim the content was rechecked

#### Scenario: Changed cached bytes
- **WHEN** checked content differs from its recorded digest
- **THEN** Saucepan reports an integrity failure without replacing the expected digest with the changed bytes
