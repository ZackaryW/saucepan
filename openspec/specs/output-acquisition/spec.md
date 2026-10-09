# output-acquisition Specification

## Purpose

Produce immutable, identifiable acquisition outputs with reproducible provenance and policy-controlled handling of repeated identical pulls.

## Requirements

### Requirement: Each published output identifies its request and resolved content
An output SHALL have its own UUID, normalized acquisition request, source UUID, resolved revision or asset identity, content digest, creation order, effective policies, and availability state. Different acquired versions SHALL have distinct entry UUIDs and coexist while retention and protection permit. Immutability SHALL prohibit replacing an entry's payload with different acquired bytes; it SHALL NOT prohibit reclaiming its content while retaining provenance under [output-policies](../output-policies/spec.md). Metadata freshness SHALL NOT substitute for verifying that requested output content is available.

#### Scenario: Two releases of one resource are acquired
- **WHEN** requests acquire release one and later release two and retention permits both
- **THEN** two independently indexed store entries exist with their actual release identities
- **AND** acquiring release two does not overwrite release one's files

### Requirement: Identical acquisition behavior is policy controlled
Requests SHALL support `reuse` and `new_entry` identical-acquisition policies, defaulting to `reuse`. Reuse SHALL require matching normalized content selection, resolved identity, and compatible effective lifecycle policies, plus available valid content. `new_entry` SHALL produce a distinct UUID even for identical content. Concurrent reuse requests SHALL converge on a compatible published entry.

#### Scenario: Identical pulls reuse an available output
- **WHEN** a request uses `reuse` and a compatible identical ready entry exists
- **THEN** that entry is returned without a new creation order or duplicate store entry

#### Scenario: Explicitly requested duplicate output
- **WHEN** a request uses `new_entry` for already acquired bytes
- **THEN** a separate output entry is published with its own policies and creation order

#### Scenario: Policy changes prevent silent mutation of shared output
- **WHEN** a repeated acquisition requests an incompatible lifecycle policy
- **THEN** it obtains a separate entry rather than changing the existing entry's policy for other claimants

### Requirement: Unsafe or incomplete content is not published
Acquisition SHALL validate integrity expectations, selected executable paths, filesystem containment, archive collisions, link handling, and resource bounds before declaring an output ready. Escaping paths or unresolved required content SHALL fail without altering existing claims or outputs.

#### Scenario: Downloaded archive attempts path traversal
- **WHEN** extraction encounters a path outside the staged output root
- **THEN** acquisition fails without writing outside that root or publishing a new ready entry

### Requirement: Core and CLI expose consistent acquisition results
The native API and CLI SHALL report the same entry UUID, content path, resolved provenance, reuse outcome, discovery freshness/fallback status, and effective policies. Unscoped acquisition SHALL NOT create a synthetic application claim.

#### Scenario: An unscoped CLI acquisition succeeds
- **WHEN** a caller acquires an item without supplying an application
- **THEN** it receives a normal indexed output and no application record or claim is invented
