# group-leases Specification

## Purpose

Let consumers retain several named acquired resources under one optional access boundary and one lifetime, without requiring persistent claims or creating a registered group.

## Requirements

### Requirement: Groups select bounded named existing resources
A group request SHALL contain 1 through 128 named members from one store and optional policy UUID references. Each member SHALL identify an existing output or claim UUID and optionally a contained directory view. Names SHALL be unique, case-sensitive ASCII matching `[A-Za-z][A-Za-z0-9_-]{0,63}`. Unknown fields, malformed identities, duplicate names and invalid views SHALL fail with invalid_request before protection is acquired. CLI request input SHALL be bounded to 1 MiB. A name SHALL NOT create a scope, persistent alias or group identity. Distinct names MAY select the same output or different views of it.

#### Scenario: One output has two named views
- **WHEN** command and hooks select different valid folders within the same output
- **THEN** both named directories are returned and the underlying output is protected once by that group

#### Scenario: Invalid group is rejected before admission
- **WHEN** a request has no members, more than 128 members, duplicate names, an unknown field or an escaping relative path
- **THEN** it fails with invalid_request without creating a live lease or starting a consumer

### Requirement: Group identity is explicit and optional
An output-only group SHALL support trusted local access without application registration, claims or credentials. An app-bound group SHALL evaluate every member under that application's current credential and authorization rules, including direct-output members. Claim targets SHALL require an explicit app context and belong to that application. Request payloads SHALL NOT select per-member identities, administrative fallback or credentials. Failed app authorization SHALL NOT retry as unrestricted local access. Public discovery SHALL NOT constitute permission to lease a resource.

#### Scenario: Unregistered consumer uses shared acquired files
- **WHEN** a local caller opens a ready output-only group without app identity or group policies
- **THEN** it receives protected paths without creating an application or claim

#### Scenario: Direct UUID does not bypass app restrictions
- **WHEN** an app-bound group includes an output denied to that app
- **THEN** admission fails for the whole group without exposing any successful member paths

#### Scenario: Claim requires ownership context
- **WHEN** an unscoped request includes a claim target or an app-bound request names another application's claim
- **THEN** it is rejected without inferring a caller or broadening access

### Requirement: Admission selects and protects the complete group consistently
Group opening SHALL select all claim outputs and evaluate all member permissions from coordinated verified metadata. It SHALL protect every distinct selected output before exposing any successful result. Verification failure, missing content, permission denial, contention or preparation failure SHALL release every protection acquired by that attempt. A relevant claim, app credential, application revision or source-policy change during verification SHALL cause a conflict or applicable denial before success; unrelated policy registration SHALL NOT alone invalidate admission. Payload scanning SHALL occur outside global metadata coordination while selected outputs remain protected. Admission waiting SHALL use a bounded total lock budget, not a fresh budget for each member.

#### Scenario: Later member fails verification
- **WHEN** two outputs are protected during preparation and the second fails full verification
- **THEN** neither directory is returned and both protections from that attempt are released

#### Scenario: Claim changes during a scan
- **WHEN** a claim updates or is released while group admission verifies selected payloads
- **THEN** the group does not return a mixture of pre-change and post-change selections and instead fails the revision check without leaked protection

#### Scenario: Groups and maintenance overlap
- **WHEN** two groups request the same outputs in opposite input orders while reclaim runs
- **THEN** they complete or report bounded contention without deadlock, partial success or deletion of a protected output

### Requirement: Admitted groups retain their exact outputs
A successful native group SHALL hold a non-serializable lifetime guard and named descriptors identifying exact outputs, selected directories and actual verification. Later updates, claim release, uninstall or policy changes SHALL NOT redirect or revoke its existing output protection. A future group SHALL use current authority and selection. Releasing a group SHALL release its distinct protections together, leaving other leases and retention/protection demands effective. Serializing or copying descriptors SHALL NOT extend lifetime or grant authority.

#### Scenario: Selection advances while a group is in use
- **WHEN** an admitted group's claim advances to a newer output and the previous output becomes reclaimable
- **THEN** the group continues using its original output and reclaim defers removal until its guard and other protections end

#### Scenario: Another group retains the same output
- **WHEN** one of two groups protecting an output is dropped
- **THEN** the remaining group still protects it without losing either of its named views

### Requirement: Opening a group has no acquisition or registration effects
Opening a group SHALL NOT retrieve sources, refresh selections, recover unavailable content, create claims, adopt policies, register groups or change revision history. Missing or reclaimed content SHALL fail using the applicable existing resource error. Existing store-open recovery and lifetime bookkeeping SHALL retain their documented behavior. Group policies SHALL govern Saucepan admission rather than sandboxing the child, blocking OS filesystem access or creating reusable authority for later operations.

#### Scenario: An unavailable member has a recovery policy
- **WHEN** a claim's selected output is unavailable even though its reclaim policy permits recovery
- **THEN** group opening reports unavailability without downloading or moving that claim

#### Scenario: Filesystem rights remain outside the group
- **WHEN** a consumer receives a group descriptor
- **THEN** documentation identifies it as protected resource addresses and does not claim that it restricts the consumer's independent OS file access
