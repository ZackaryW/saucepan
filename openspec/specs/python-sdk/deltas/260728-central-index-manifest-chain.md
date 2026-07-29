## ADDED Requirements

### Requirement: Sauce snapshots expose manifest source
A `Sauce` snapshot SHALL expose the source that supplied its manifest, so a caller can distinguish an entry described by its own repository from one described by a central index without parsing raw state.

#### Scenario: Reading manifest source from a snapshot
- **WHEN** a caller reads a sauce whose manifest came from a central index
- **THEN** the snapshot reports the index as its manifest source

#### Scenario: Repository-sourced snapshot
- **WHEN** a caller reads a sauce whose manifest came from its own repository
- **THEN** the snapshot reports the repository as its manifest source

### Requirement: Bucket stubs expose extra fields
Parsed bucket stubs SHALL expose fields beyond the required ones to callers, so consumer data carried in an index is reachable through the SDK rather than only through raw state reads.

#### Scenario: Stub carrying consumer data
- **WHEN** a caller reads stubs from a bucket whose entries carry extra fields
- **THEN** those fields are present on the returned stubs

#### Scenario: Stub without extra fields
- **WHEN** a caller reads stubs from a bucket whose entries carry only the required fields
- **THEN** the stubs expose an empty set of extra fields rather than failing

### Requirement: Additive SDK compatibility
The additions in this change SHALL be additive. Existing SDK call signatures, return types, cache invalidation behavior, and the documented exit-code to exception mapping SHALL remain unchanged.

#### Scenario: Existing caller is unaffected
- **WHEN** an existing caller written against the prior SDK runs against this version
- **THEN** every existing call behaves identically and no exception mapping changes

### Requirement: Runtime dependency boundary is preserved
The SDK SHALL remain standard-library-only and independently vendorable after these additions, importing neither the platform resolver nor any third-party package.

#### Scenario: Import check after the change
- **WHEN** the SDK modules are walked for absolute top-level imports
- **THEN** every one resolves to the standard library, and all other imports are package-relative
