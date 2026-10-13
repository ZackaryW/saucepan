## MODIFIED Requirements

### Requirement: Each published output identifies its request and resolved content
A managed output SHALL have its own UUID, normalized acquisition request, source UUID, resolved revision or asset identity, content digest, creation order, effective policies, and availability state. Different acquired versions SHALL have distinct entry UUIDs and coexist while retention and protection permit. Immutability SHALL prohibit replacing an entry's payload with different acquired bytes; it SHALL NOT prohibit reclaiming its content while retaining provenance under [output-policies](../output-policies/spec.md). Metadata freshness SHALL NOT substitute for verifying that requested output content is available. Temporary and caller-owned acquisitions SHALL instead return explicit ownership, path and source evidence without a persistent entry UUID or managed history.

#### Scenario: Two releases of one resource are acquired
- **WHEN** requests acquire release one and later release two and retention permits both
- **THEN** two independently indexed store entries exist with their actual release identities
- **AND** acquiring release two does not overwrite release one's files

### Requirement: Identical acquisition behavior is policy controlled
Requests SHALL support `reuse` and `new_entry` identical-acquisition policies, defaulting to `reuse`. Managed reuse SHALL require matching normalized content selection and compatible effective policies, plus available valid content. Ordinarily the resolved identity SHALL match. During an explicit acquisition update, equivalent selected manifests and relevant dependency content MAY preserve the previous selected output while separately reporting the newly checked source identity, as specified by acquisition-refresh. This SHALL not rewrite the reused output provenance. `new_entry` SHALL produce a distinct UUID even for identical content. Concurrent reuse requests SHALL converge on a compatible published entry.

#### Scenario: Identical pulls reuse an available output
- **WHEN** a request uses `reuse` and a compatible identical ready entry exists
- **THEN** that entry is returned without a new creation order or duplicate store entry

#### Scenario: Explicitly requested duplicate output
- **WHEN** a request uses `new_entry` for already acquired bytes
- **THEN** a separate output entry is published with its own policies and creation order

#### Scenario: Policy changes prevent silent mutation of shared output
- **WHEN** a repeated acquisition requests an incompatible lifecycle policy
- **THEN** it obtains a separate entry rather than changing the existing entry's policy for other claimants

### Requirement: Core and CLI expose consistent acquisition results
The native API and CLI SHALL report equivalent ownership, content path, selected and checked provenance, reuse/change outcomes, verification evidence and effective policies. Managed results SHALL additionally report the persistent entry UUID and revision/reclaim outcome. Temporary and caller-owned results SHALL not fabricate those managed identities. Unscoped acquisition SHALL NOT create a synthetic application claim.

#### Scenario: An unscoped CLI acquisition succeeds
- **WHEN** a caller uses managed acquisition without supplying an application
- **THEN** it receives a normal indexed output and no application record or claim is invented

## ADDED Requirements

### Requirement: Acquisition consumes retrieved material through explicit destinations
Acquisition SHALL consume a source retrieval and apply copy, export, extraction and subpath selection to produce resolved files. The materialization stage SHALL perform no provider fetching; the acquisition update orchestrator MAY request another retrieval before materializing when update policy requires it. Managed, temporary and caller-owned directory destinations SHALL be explicit. Source and destination SHALL not overlap, and copying SHALL not create mutable aliases to source data. Caller destinations SHALL be create-new and SHALL not be subsequently managed or reclaimed by Saucepan. Incompatible temporary/no_store retrieval plus managed output, persistent claim or managed revision history SHALL fail before effects. Cached retrieval MAY feed any supported output destination. Existing one-call managed APIs SHALL compose retrieval and acquisition without forcing their usage on new callers.

#### Scenario: Git subpath becomes copied output
- **WHEN** acquisition selects plugins/tool from retrieved commit C
- **THEN** it returns an independent copied tree for that subpath without a .git directory
- **AND** later repository changes cannot modify those returned bytes through shared links

#### Scenario: Cached ZIP supplies a caller-owned extraction
- **WHEN** acquisition extracts a cached ZIP to an absent caller-owned directory
- **THEN** the result has the selected files without a managed output record, claim or revision history

#### Scenario: Destination exists
- **WHEN** a caller-owned acquisition targets an occupied directory
- **THEN** it fails before replacing that directory or its files
