## MODIFIED Requirements

### Requirement: Integrity boundaries remain mandatory
Managed acquisition publication, compatible-output reuse, staged-publication recovery, activation, and rollback SHALL retain their existing integrity requirements independently of access-time defaults. Full validation before a probe and before committing an activation SHALL continue to reject invalid candidates while preserving the prior active binding. Source-discovery freshness SHALL NOT be used as evidence of stored content integrity.

#### Scenario: Fast lookup does not authorize corrupted reuse
- **WHEN** metadata lookup succeeds for an existing output whose bytes have been corrupted and a later acquisition attempts to reuse it
- **THEN** acquisition does not return those bytes as a valid reusable output

#### Scenario: A probe modifies the candidate
- **WHEN** the candidate passes its initial validation but its probe changes recorded content
- **THEN** activation fails its final integrity check and preserves the previous binding

## ADDED Requirements

### Requirement: Optional payload verification is explicit and truthfully reported
Retrieval SHALL support none, if_available and required expected-digest verification, defaulting to if_available. Required SHALL fail if no supported expected digest can be checked, and any attempted mismatch SHALL fail. None plus an explicit expected digest SHALL be rejected as contradictory. Results SHALL distinguish unchecked content, checked expected digest, computed content identity and Git object validation. Stronger verification requests SHALL not reuse weaker evidence without checking the material. Temporary and caller-owned operations SHALL not require whole-payload hashing solely to manufacture a managed identity. Managed manifest hashing, metadata integrity and structural validation SHALL remain explicitly documented requirements of their selected operations; they SHALL not be represented as optional upstream authenticity checks.

#### Scenario: Optional verification is disabled for fresh file delivery
- **WHEN** no_store retrieval of a raw file uses none with no expected digest and no other digest-requiring option
- **THEN** it delivers the bounded file without a whole-payload checksum pass and reports verification as unchecked

#### Scenario: Required digest has no trusted expected value
- **WHEN** required is selected but no supported expected digest is supplied or available from the source
- **THEN** retrieval fails rather than report its own newly computed checksum as successful expected-content verification

#### Scenario: Verification does not disable containment
- **WHEN** a ZIP with an escaping entry is acquired after none verification
- **THEN** extraction still rejects the unsafe path before publication
