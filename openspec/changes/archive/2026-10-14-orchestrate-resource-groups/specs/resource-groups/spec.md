# Spec Delta

## Purpose

Let consumers persist a bounded resource set, explicitly prepare its content, and publish and consume a coherent selected generation under existing policies.

## ADDED Requirements

### Requirement: Registered groups have explicit optional ownership and stable identity
The system SHALL register groups by UUID and a nonempty owner-local registration key of at most 4096 UTF-8 bytes. A group SHALL belong to trusted local administration or one explicit application. Local groups SHALL require no application, claim or credential. App groups SHALL enforce current credentials and member source/action/scope restrictions without local fallback. Equal registrations SHALL converge; conflicting definitions for one key SHALL fail. Lookup SHALL support UUID and owner-local key without inferring an owner. Replacement and removal SHALL require the expected group revision. Replacement SHALL invalidate the prepared generation without releasing independent claims or live leases. Removal SHALL remove only group metadata. App retirement SHALL remove its group registrations without affecting retained consumers.

#### Scenario: Repeat local registration
- **WHEN** two local callers register the same key and definition
- **THEN** one group UUID exists without an application or claim being synthesized

#### Scenario: Conflicting replacement
- **WHEN** a caller replaces a group using a revision superseded by another mutation
- **THEN** replacement fails without changing the definition or prepared generation

#### Scenario: Remove an in-use group
- **WHEN** a registered group is removed while a consumer retains its outputs
- **THEN** later lookup cannot find the registration and the consumer's guards remain effective

### Requirement: Group definitions contain bounded explicit resource intents
A definition SHALL contain 1 through 128 uniquely named members using existing group name and relative-view validation. Each target SHALL be an existing managed output, an owned claim, or a managed acquisition intent naming a registered source, selector, selection and flat policy references. Local acquisition intents SHALL create no claims. App acquisition intents SHALL include explicit claim scope and context. Claim targets SHALL require the group's application context. Unknown fields, invalid references, mixed owners, incompatible destinations and temporary/no_store behavior SHALL fail before provider effects. CLI definition input SHALL be limited to 1 MiB. Registration SHALL perform no retrieval or acquisition. Member names SHALL NOT imply scopes, alias claims or establish filesystem authority.

#### Scenario: Register now and prepare later
- **WHEN** a valid acquisition intent is registered while its provider is offline
- **THEN** registration succeeds using local metadata and no network request or output is created

#### Scenario: Unsupported persistent destination
- **WHEN** an acquisition member resolves to temporary or no_store retrieval
- **THEN** registration or preparation rejects the policy combination before fetching and the standalone fresh APIs remain available

### Requirement: Explicit preparation evaluates member update and recovery policies
Preparation SHALL obtain missing acquisition members and evaluate existing member update/reclaim policies before reusing or replacing selections. Retrieve SHALL operate on provider source material; acquisition SHALL copy, export or extract selected files. Manual acquisition updates and 86400-second source freshness SHALL remain defaults. Existing fixed output targets SHALL not move or recover implicitly. Existing claim targets SHALL select their current output and recover missing content only as their reclaim policy permits. Compatible Git source/selector resolutions prepared together SHALL use one captured commit for the members actually being materialized; intentionally retained manual members SHALL preserve their earlier output. Exact provenance and permitted stale fallback SHALL be reported per member. Failed origin checks SHALL NOT become successful-check evidence.

#### Scenario: Two folders update from one repository
- **WHEN** both due acquisition members select folders from a compatible moving Git branch
- **THEN** their new copied results use the same resolved commit even if the origin advances during preparation

#### Scenario: A member requires explicit recovery
- **WHEN** a required claim's output is missing and its reclaim policy reports unavailability
- **THEN** preparation fails without changing any claim or group selection

#### Scenario: Preparation respects manual selection
- **WHEN** one member's acquisition update is due and another available member is manual
- **THEN** preparation can update the due member while retaining and reporting the manual member's exact earlier selection

### Requirement: Selected generations publish as a complete set
Successful preparation SHALL publish all named selections, affected claim mutations and successful acquisition-check evidence in one recoverable metadata transition. It SHALL revalidate group revision, relevant policy attachments, application authority, claim observations and candidate availability before publication. A failed or superseded attempt SHALL leave the previous group generation, claims and successful-check state unchanged. Complete source caches and unclaimed output candidates SHALL be permitted to remain after failure and SHALL be subject to existing reclaim policies. A recovered committed transition SHALL expose the complete new generation; an uncommitted attempt SHALL leave the complete old generation. Existing selected output payloads SHALL never be overwritten. External provider state and consumer hooks SHALL remain outside the transaction.

#### Scenario: A later member fails
- **WHEN** one candidate is complete and a second member fails acquisition or validation
- **THEN** no claim or selected generation advances, although complete cache material may remain

#### Scenario: Crash during final publication
- **WHEN** publication is interrupted and store recovery runs
- **THEN** all selected members and participating claims agree on the old or complete committed new transition, never a partial set

#### Scenario: Authority changes while fetching
- **WHEN** a relevant policy, credential, group revision or participating claim changes during preparation
- **THEN** the attempt conflicts or is denied without restoring stale state or recording successful checks

### Requirement: Opening a registered generation remains read-only and retained
Opening or running a registered group SHALL use current authority and one exact prepared generation without retrieving, refreshing or creating claims. Missing generation/content or changed claim witnesses SHALL fail explicitly rather than silently mixing selections. All distinct outputs SHALL be protected before successful return. Native prepare-and-open and CLI prepare-and-run SHALL retain the exact prepared outputs across publication and launch without a reclaim gap. Native guards and supported inherited process handles SHALL retain resources; serialized descriptors and unpinned preparation-only CLI results SHALL NOT promise retained paths. Explicit prepare-and-pin SHALL instead follow the durable output-pins contract. Later publication, removal or uninstall SHALL NOT redirect or revoke an admitted group's lifetime. Persistent group references alone SHALL NOT prevent normal revision reclaim.

#### Scenario: A claim advances independently
- **WHEN** a claim named in the saved generation now selects a different output
- **THEN** opening that generation reports a conflict and explicit preparation is required to reconcile it

#### Scenario: Reclaim races the launch handoff
- **WHEN** prepare-and-run succeeds while reclaim runs concurrently
- **THEN** every selected output remains protected continuously until supported consumer lifetime protection ends

#### Scenario: Read-only launch encounters an unprepared registration
- **WHEN** a caller runs a newly registered group without preparing it
- **THEN** the command reports that preparation is required and does not start the consumer or fetch content

### Requirement: Group management is available consistently through native and CLI interfaces
Native and CLI interfaces SHALL expose registration, UUID/key lookup, listing, revision-checked replacement/removal, explicit preparation, read-only retained consumption and explicitly named prepare-and-run composition. Metadata results SHALL identify group revision, selected generation, exact member outputs, claim witnesses, availability and provenance. Commands SHALL use existing default-root and explicit-app conventions. Errors SHALL identify an applicable stable error code and failing member when known, with no successful partial result or consumer start. Existing inline run-group requests SHALL keep their read-only contract.

#### Scenario: Local consumer uses saved intent
- **WHEN** a local caller registers, prepares and runs a group without --as-app
- **THEN** the selected files are available through the supported group descriptor without application registration

### Requirement: Durable generation retention is independently opt-in
Group registration, lookup and ordinary preparation SHALL NOT implicitly pin outputs. An explicit group-pin operation SHALL retain one complete selected generation under the output-pins contract. Explicit preparation with a pin request SHALL publish the pin with the selected generation and return its identity and exact output paths after commit. Subsequent group update/removal or claim release SHALL not release that pin. Returning a new generation SHALL not redirect a consumer's already pinned paths; old and new consumers SHALL be able to retain different generations independently.

#### Scenario: Host keeps an old plugin generation alive
- **WHEN** a host prepares and pins a newer group generation while an old one remains pinned
- **THEN** both generations remain available and the host explicitly releases the old pin only after its users finish
