## Purpose

Define central indexes as a curated manifest source: how one is registered and fetched, its entry schema and backward-compatibility guarantee, the ref requirement on manifest-supplying entries, precedence among indexes, and behavior when an index is unreachable.

## Requirements

### Requirement: A central index is a repository target
A remote central index SHALL be registered as a repository target and fetched through the existing Git and gh machinery. Saucepan SHALL NOT introduce an HTTP client for index retrieval.

#### Scenario: Registering a remote index
- **WHEN** a user registers a central index by repository target
- **THEN** Saucepan fetches it using the same clone and pull path used for sauces, inheriting configured authentication

#### Scenario: Private index
- **WHEN** a registered index repository is private and the configured source can authenticate
- **THEN** Saucepan fetches it without additional credential configuration

#### Scenario: Local index still supported
- **WHEN** an index is registered by local path or `file://` URL
- **THEN** Saucepan reads it directly as before

### Requirement: Index entries may be pinned
Because an index is a repository target, it SHALL accept a ref, and Saucepan SHALL record the index's resolved commit so the index state that produced a manifest is identifiable afterwards.

#### Scenario: Pinning an index
- **WHEN** an index is registered with an explicit ref
- **THEN** that ref is resolved and recorded, and later reads use the pinned state until it is changed

### Requirement: Index entries carry extra fields
An index entry SHALL preserve fields beyond the required ones, so it can carry a complete manifest document and arbitrary consumer data. Preserved fields SHALL be visible to the state-reading and search commands.

#### Scenario: Entry carrying a manifest
- **WHEN** an index entry carries a full manifest document in its extra fields
- **THEN** that manifest is available to the resolution chain

#### Scenario: Consumer data survives
- **WHEN** an index entry carries fields Saucepan does not interpret
- **THEN** those fields are preserved verbatim and are readable by callers

### Requirement: Index schema remains backward compatible
`name`, `version`, and `url` SHALL remain required on every index entry. An index written for this version SHALL remain parseable by binaries predating this change.

#### Scenario: Older binary reads a newer index
- **WHEN** a binary predating this change reads an index containing entries with extra fields
- **THEN** it parses the entries successfully and ignores what it does not recognize

#### Scenario: Entry missing a required field
- **WHEN** an index entry omits `name`, `version`, or `url`
- **THEN** Saucepan rejects the index as invalid

### Requirement: A manifest-supplying entry must supply a ref
An index entry that supplies a manifest SHALL also supply the ref the manifest describes, and Saucepan SHALL check out that ref during fetch, so that the recorded version and the working tree cannot disagree.

#### Scenario: Installing from an index-supplied manifest
- **WHEN** an index entry supplies a manifest and a ref for a target
- **THEN** Saucepan checks out that ref and the recorded version matches the checked-out tree

#### Scenario: Manifest without a ref
- **WHEN** an index entry supplies a manifest but no ref
- **THEN** Saucepan rejects that entry as invalid and continues the chain

### Requirement: Unreachable indexes are skipped
An index that cannot be fetched or read SHALL produce a warning and be skipped. It SHALL NOT fail an operation that another link of the chain can satisfy.

#### Scenario: One index is unreachable
- **WHEN** one registered index cannot be fetched and another describes the target
- **THEN** Saucepan warns about the unreachable index and installs using the reachable one

#### Scenario: All indexes unreachable and no repository manifest
- **WHEN** every registered index is unreachable and the target has no root manifest
- **THEN** Saucepan warns for each and returns a not-found result

### Requirement: Index precedence is deterministic
When more than one registered index describes the same target, Saucepan SHALL resolve using registration order and SHALL emit a warning on stderr during every resolution that encounters a later matching entry. The warning SHALL identify the shadowed and winning indexes.

#### Scenario: Two indexes describe one target
- **WHEN** two registered indexes both describe the same target with different manifests
- **THEN** the earlier-registered index supplies the manifest and Saucepan warns on stderr that the later entry was shadowed

#### Scenario: Collision is reported on a later resolution
- **WHEN** the same colliding registrations are consulted by a subsequent install or update resolution
- **THEN** Saucepan emits the shadowing warning again rather than treating an earlier warning as persistent state
