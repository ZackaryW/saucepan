# Spec Delta

## Purpose

Let consumers choose the integrity work performed during resource access while preserving mandatory validation when content is acquired or activated.

## ADDED Requirements

### Requirement: Access-time verification is explicit
Resource access SHALL support `metadata` and `full` verification modes. Ordinary path resolution and routine managed launch SHALL default to metadata mode. Metadata mode SHALL validate record identity, output kind, ready state, root existence/type, and filesystem containment without traversing or reading output payload files. A selected managed executable SHALL additionally be declared in the output and present as a regular contained file. Unknown modes SHALL fail before launching a consumer.

#### Scenario: Looking up a large output
- **WHEN** a caller resolves a ready output using the default mode with no pending recovery
- **THEN** Saucepan returns its exact directory without scanning the output tree or hashing payload bytes
- **AND** the result identifies verification as `metadata`, not full content verification

#### Scenario: Unsafe or unavailable target
- **WHEN** an entry is pending reclaim, reclaimed, missing its root, or resolves through a forbidden filesystem link
- **THEN** metadata access fails without refreshing or repairing the source

### Requirement: Full verification checks the recorded complete manifest
Full mode and the dedicated `verify ENTRY_UUID` command SHALL compare the current complete output tree with the recorded manifest, including file digests, paths, entry types, and supported executable metadata. Full verification SHALL reject missing, extra, or changed content and propagate read/bounds errors without silently downgrading to metadata mode. It SHALL hold lifetime protection throughout the check and report the entry UUID and successful verification level or a useful failure diagnostic. Verification SHALL NOT rewrite the manifest to accept changed bytes.

#### Scenario: Payload is changed without changing its path
- **WHEN** a file is modified after acquisition and the caller requests full verification
- **THEN** verification fails even if the output is still recorded as ready and the filename is unchanged
- **AND** metadata-only directory resolution is not represented as detecting that payload modification

#### Scenario: Full check fails before starting a consumer
- **WHEN** a leased run requests full mode and verification finds an unexpected or missing file
- **THEN** the consumer is not started and the temporary lease is released

### Requirement: Integrity boundaries remain mandatory
Acquisition publication, compatible-output reuse, staged-publication recovery, activation, and rollback SHALL retain their existing integrity requirements independently of access-time defaults. Full validation before a probe and before committing an activation SHALL continue to reject invalid candidates while preserving the prior active binding. Source-discovery freshness SHALL NOT be used as evidence of stored content integrity.

#### Scenario: Fast lookup does not authorize corrupted reuse
- **WHEN** metadata lookup succeeds for an existing output whose bytes have been corrupted and a later acquisition attempts to reuse it
- **THEN** acquisition does not return those bytes as a valid reusable output

#### Scenario: A probe modifies the candidate
- **WHEN** the candidate passes its initial validation but its probe changes recorded content
- **THEN** activation fails its final integrity check and preserves the previous binding

### Requirement: Verification does not change resource identity or serialize unrelated use
Access-time mode SHALL NOT change entry identity, item keys, retention counts, creation order, claims, or source freshness. During an access-time full scan, reclaim SHALL defer deletion of that entry, while unrelated normal metadata access and index mutations SHALL NOT wait for the entire scan to finish. Recovery already required before an operation is a separate integrity boundary and SHALL NOT be skipped to meet the lightweight-access behavior.

#### Scenario: Full scan and unrelated lookup overlap
- **WHEN** one full verification is paused while reading content and another caller accesses an unrelated ready entry
- **THEN** the unrelated lookup can complete before the scan resumes
- **AND** concurrent reclaim cannot delete the entry being verified

#### Scenario: Same output with different access modes
- **WHEN** a consumer resolves an unchanged entry once with metadata mode and once with full mode
- **THEN** both identify the same entry and neither adds an acquisition or advances discovery freshness
