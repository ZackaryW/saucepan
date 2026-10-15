# Spec Delta

## ADDED Requirements

### Requirement: Profile-assisted setup uses existing managed publication
Managed installation SHALL accept ordinarily registered profile-expanded policy references through its existing request contract. Installation SHALL preflight managed storage and usable-app protection, rejecting temporary or no-store delivery and conflicting constraints before provider work. It SHALL preserve explicit source/selector/platform, executable, bounded probe, scope and shim inputs. A profile SHALL NOT invent application scope, enable credentials or replace current authority. Setup SHALL use existing lifecycle receipt, candidate protection, validation, publication and reconciliation semantics. Registration of reusable policies/sources and reusable cached material SHALL remain separate from successful live application publication.

#### Scenario: Install a managed executable from a profile
- **WHEN** a caller supplies a valid installation request using registered for-install profile refs, an explicit scope, probe and built shim
- **THEN** successful lifecycle publication identifies the app, claim, selected output, launcher and operation with the same guarantees as a hand-assembled policy request

#### Scenario: Candidate fails after setup preparation
- **WHEN** profile policies and a source are registered but installation's candidate fails its probe
- **THEN** no live partially installed application is published and prior active selections remain usable
- **AND** reusable registrations and cached content are not misreported as a completed installation

#### Scenario: Temporary request bypasses profile preparation
- **WHEN** a caller directly supplies temporary cache policy to managed installation
- **THEN** installation rejects it before retrieval rather than publishing a stable launcher pointing to temporary content

### Requirement: Setup retries preserve expanded identities
A retry under an existing lifecycle operation UUID SHALL retain the originally submitted expanded refs and input identity. A changed profile expansion or payload under that operation UUID SHALL conflict under the existing receipt contract rather than reinterpret the pending operation. Existing applications SHALL change adopted behavior only through explicit authorized adoption. Profile listing or SDK construction SHALL never resume setup, update bindings, modify PATH or create applications.

#### Scenario: Profile changes while setup is interrupted
- **WHEN** a caller retries an interrupted setup using the original operation UUID but different expanded policy refs
- **THEN** the operation rejects changed input and preserves the original receipt for inspection or reconciliation
