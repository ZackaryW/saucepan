## Purpose

Control automatic freshness checks for acquired selections independently of source-cache policy and retained revision history, while preserving truthful provenance and scoped ownership.

## ADDED Requirements

### Requirement: Acquisition updates are an independent demand-driven policy
Acquisition SHALL support manual, always and max_age update policies, defaulting to manual for existing managed convenience behavior. A due update SHALL be evaluated before reuse of an available output or claim. It SHALL cause fresh retrieval for a moving selector even if ordinary source-cache freshness has not expired, or consume fresh retrieval evidence produced for that same operation. It SHALL not fetch twice for one satisfied check. Exact commit selectors SHALL remain pinned. Bare path access, ordinary queries and leases SHALL not perform update checks. No background timer is implied. Contradictory cache-only and due-update requirements SHALL fail before effects.

#### Scenario: Source cache is fresh but acquisition update is due
- **WHEN** an acquired folder has an always update policy and its Git branch advances within the source's normal cache interval
- **THEN** acquisition retrieves the current branch commit and evaluates that folder before returning
- **AND** it does not return the previous selection solely because the source cache was fresh

#### Scenario: Manual selection remains stable
- **WHEN** a compatible available claim with manual updates is ensured again
- **THEN** the selected output is reused without checking the remote head

### Requirement: Selected content determines whether an automatic update creates a revision
For compatible reuse policy, an acquisition update SHALL compare the selected files, paths, executable metadata and relevant dependency content. A changed repository head alone SHALL not require a new acquired revision when the selected subpath remains equivalent. An unchanged result SHALL report the checked source provenance separately from the original selected output provenance and SHALL not rewrite that output's identity, creation order or manifest. Changed selected content SHALL produce a new immutable output before changing selection. Explicit new_entry SHALL still allocate a distinct output even for equivalent selected content.

#### Scenario: Only another Git folder changed
- **WHEN** a fresh commit changes docs but the acquisition selects an unchanged plugins/tool folder
- **THEN** compatible reuse returns the existing output with changed false and separate checked and selected commits
- **AND** it does not invent new historical bytes or relabel the existing output as the new commit

#### Scenario: Selected subpath changed
- **WHEN** the selected folder or its relevant dependency content changes at the newly retrieved commit
- **THEN** acquisition validates and publishes a new copied revision and applies its revision policy
- **AND** previously acquired bytes are not overwritten

### Requirement: Update checks preserve previous results on failure and conflict
Successful acquisition-check time and observed provenance SHALL be recorded only after a valid unchanged result or successful changed-result publication. Failed provider checks, verification, copying or publication SHALL preserve prior selection and successful-check time. Explicit stale fallback SHALL report stale update evidence without claiming a successful new check. Concurrent scoped updates SHALL obey application revision, context, release and uninstall fences. Automatic app updates SHALL require the update action and allowed policy; changing a claim SHALL not implicitly change an active binding.

#### Scenario: Failed automatic update
- **WHEN** the provider is unreachable during a due update and stale fallback is disabled
- **THEN** acquisition fails while the prior output, claim selection and successful-check timestamp remain unchanged

#### Scenario: Uninstall overtakes an automatic check
- **WHEN** uninstall commits while a due acquisition update prepares copied content
- **THEN** the update cannot recreate the registry or publish a new live claim

#### Scenario: Changed output belongs to an active application
- **WHEN** an automatic primitive acquisition changes that application's claim selection
- **THEN** its existing active binding remains unchanged until an explicit managed activation transition

### Requirement: Check state is independent from shared bytes and revision history
Managed check state SHALL identify its acquisition intent, policy and selected output. Explicit app scopes SHALL retain independent selections and check state even when they share outputs. Unclaimed managed requests SHALL coordinate equivalent intents without synthesizing an app. A policy change SHALL not inherit incompatible freshness evidence. Repeated concurrent due requests for the same intent SHALL coordinate checking and publication without duplicate claims or incompatible state. Changing revision retention SHALL not change the configured source freshness or acquisition update interval, or erase another scope's check state. Distinct effective policy intents MAY require their own initial successful check.

#### Scenario: Two scopes share bytes but update independently
- **WHEN** scope A performs a due automatic update while scope B still selects the original shared output
- **THEN** only A's selection and check state advance
- **AND** B remains governed by its own update and revision policies
