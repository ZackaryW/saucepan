# Spec Delta

## ADDED Requirements

### Requirement: Effective policy explanation is authorized and read-only
Native and CLI interfaces SHALL explain effective policies for typed retrieval, acquisition, composed and existing-claim inputs without provider work, resource publication, probing, registration or adoption. Applicable axes SHALL report effective values, origin level and definition UUID/digest or built-in default, plus applicable restriction and constraint evidence. Inapplicable axes SHALL be distinguished from executed settings. Existing claim explanations SHALL distinguish adopted behavior from current restrictions. Explanations SHALL use the same resolution rules as execution and a coordinated verified snapshot for persisted inputs. They SHALL NOT become editable authority or a guarantee of future authorization, content availability or successful execution. Ordinary store-open recovery SHALL retain its current contract.

#### Scenario: Explain a customized recipe
- **WHEN** an authorized caller explains a composed request with profile-expanded refs and an explicit latest-three policy
- **THEN** the result identifies its actual leaf UUIDs, effective revision count and applicable default verification separately without contacting the provider

#### Scenario: Inspect a surviving claim after app defaults change
- **WHEN** an authorized caller explains a claim whose app defaults have since changed
- **THEN** the explanation retains the claim's adopted behavior and separately reports current restrictions rather than adopting the new defaults

### Requirement: Explanation preserves portable and scoped boundaries
Inline portable explanations SHALL not require a root, home, registration or keyring. Persisted explanations SHALL enforce current read authorization for every referenced resource and applicable credentials without exposing protected definitions through a denial. Unknown, wrong-target or conflicting references SHALL produce the existing structured validation errors; missing or corrupt expected records SHALL produce integrity_error. Current restrictions SHALL be re-evaluated by a later actual operation regardless of an earlier explanation.

#### Scenario: Denied app attempts to inspect another scope
- **WHEN** an app-bound explanation requests a claim outside its permitted scope
- **THEN** it returns access_denied with no protected explanation payload and no unbound retry

#### Scenario: Root-free temporary preview
- **WHEN** a caller explains an inline temporary retrieval using only its portable catalog
- **THEN** it receives effective settings without looking up or creating a store
