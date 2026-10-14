# Spec Delta

## Purpose

Allow consumers to retain exact immutable managed output paths beyond process lifetime through independently owned pins that end only on explicit release.

## ADDED Requirements

### Requirement: Pins have explicit independent ownership and immutable targets
A pin SHALL identify one owner, an owner-local registration key, 1 through 128 distinct ready managed output UUIDs, its registered policy references and active/released state. Local pins SHALL require no app registration, claim or credentials. Application pins SHALL require explicit app context, current configured credentials, authorized access to every target and pin permission under current source/scope restrictions. A key SHALL be a literal nonempty string of at most 4096 UTF-8 bytes. Same-owner equivalent active registrations SHALL converge; differing requests under the same key SHALL conflict. Different owners or keys SHALL retain independently releasable pins. Targets SHALL never follow a moving claim or group selection. Unknown fields, duplicate/invalid UUIDs and invalid policy roles SHALL fail before effects; CLI pin requests SHALL be bounded to 1 MiB.

#### Scenario: Two consumers share one output
- **WHEN** two consumers create separate pins for output A and one releases its pin
- **THEN** A remains protected by the other consumer's active pin

#### Scenario: Retry a pin registration
- **WHEN** a caller repeats the same owner, key, targets and policy references
- **THEN** the existing pin is returned without adding a second retention obligation

#### Scenario: App tries to pin an inaccessible output
- **WHEN** an app names a known output UUID outside its current authority
- **THEN** pin creation is denied without local fallback or partially pinned targets

### Requirement: Pin admission is coordinated with reclaim
Admission SHALL validate availability and required path verification for all targets, retain them while verifying, recheck relevant authority and publish the complete pin atomically before reporting success. Reclaim and pin publication SHALL be coordinated so no successful pin references content deleted by a concurrent Saucepan reclaim. Reclaimed or reclaim-pending outputs SHALL fail admission without implicit recovery. An admission failure SHALL publish no pin and release its temporary guards. Corrupt expected pin metadata SHALL fail integrity validation rather than being interpreted as absent protection.

#### Scenario: Reclaim wins admission race
- **WHEN** reclaim makes an output unavailable before pin admission can protect it
- **THEN** pin admission fails without returning a successfully pinned path

#### Scenario: Pin wins reclaim race
- **WHEN** an active pin is durably published before reclaim checks the output
- **THEN** reclaim preserves its payload even after the pinning process exits

#### Scenario: A later target fails verification
- **WHEN** one of several pin targets fails required verification
- **THEN** none is newly pinned and temporary guards from that attempt are released

### Requirement: Pins retain exact immutable revisions until explicit release
An active pin SHALL prevent Saucepan from reclaiming its exact outputs independently of ordinary revision history, process lifetime, claims, group membership and active executable bindings. Updating a claim or group SHALL NOT redirect or release existing pins. Changed acquired bytes SHALL receive another output identity; compatible unchanged content SHALL retain existing reuse semantics. Claim release, app uninstall, group removal and permission changes SHALL NOT implicitly release pins. Pins SHALL not authorize external mutation, source-cache retention or access bypass. Consumers SHALL be instructed to keep mutable runtime data outside managed payloads.

#### Scenario: Plugin update preserves an older instance
- **WHEN** Power Eagle pins A, then pins newly acquired B and selects B for new instances
- **THEN** an existing instance can continue reading A's exact directory until A's pin is explicitly released

#### Scenario: Process exits and store reopens
- **WHEN** the pinning process exits and another process opens the store and reclaims
- **THEN** verified active pin membership still protects the output without a heartbeat or native live handle

#### Scenario: Policy permission narrows
- **WHEN** an app's current read permission is revoked after it creates a pin
- **THEN** new app-bound reads enforce that denial while the pin still prevents reclaim until explicitly released

### Requirement: Pin release and inspection remain explicit and owner-scoped
Native and CLI interfaces SHALL support pin creation, UUID/key inspection, owner-scoped listing and explicit release. Active-pin results SHALL identify exact output paths, availability, owner, state and policy evidence; paths SHALL not act as authorization tokens. App inspection SHALL apply current read/source/scope rules using retained witnesses without requiring a mutable claim to still point at that revision. Release SHALL require current owner credentials and unpin authority, but SHALL not require available payload or a surviving claim. Trusted local administration SHALL be able to explicitly inspect/release orphaned pins. Known released pins SHALL remain inspectable receipts; repeated authorized release SHALL be a no-op, unknown IDs SHALL fail and released keys SHALL not silently reactivate. Release SHALL not immediately delete content or override other protections.

#### Scenario: App uninstalled with pins still present
- **WHEN** an application is uninstalled before its pin is released
- **THEN** local administration can identify and explicitly release its retained pin without an automatic loss of protected bytes

#### Scenario: Release after claim moves
- **WHEN** an authorized owner releases A's pin after its claim has moved to B
- **THEN** A's pin is released without changing the claim or B's pin, and subsequent reclaim follows remaining retention demands

### Requirement: Group generation pins are explicit and failure-atomic
Pinning a prepared group SHALL snapshot all distinct output identities from one expected group revision under current authority. Group names and claim IDs SHALL remain informational provenance, not moving retention targets. Explicit prepare-and-pin SHALL publish its pin, selected generation and affected claim/check changes in the same recoverable metadata transition, with temporary protection continuous until durable publication. It SHALL leave earlier pins unchanged. Failure SHALL publish neither a partial pin nor a partial selected generation. Retrying the same committed operation key and canonical request SHALL return its exact receipt under current authorization without repeating provider work; a different request under that key SHALL conflict. An unpinned preparation or read-only lease SHALL remain unpinned.

#### Scenario: Crash during prepare-and-pin publication
- **WHEN** the process stops during final metadata publication and recovery runs
- **THEN** the previous state survives or the complete generation, claims and active pin are recovered together

#### Scenario: A lost response is retried
- **WHEN** prepare-and-pin committed but its caller retries the same key/request after losing the response
- **THEN** the exact committed result is returned without another new-entry acquisition or claiming it is a later current generation

#### Scenario: Group pinning uses a stale revision
- **WHEN** the group changes before a standalone pin-group request can publish
- **THEN** the request conflicts without pinning mixed or unexpected outputs
