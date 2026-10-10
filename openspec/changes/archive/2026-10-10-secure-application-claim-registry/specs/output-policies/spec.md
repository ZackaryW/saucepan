# Spec Delta

## ADDED Requirements

### Requirement: Applications register policy defaults and permitted overrides
An application claim registry SHALL expose typed policy defaults and explicit permitted override values or bounds. Claim acquisition SHALL validate supplied overrides before acquisition side effects and persist both overrides and effective policies. Unknown policy types and overrides outside the registration SHALL fail with `policy_conflict` or `invalid_request`. Policy registration changes SHALL require the expected application revision and applicable credentials, and SHALL reject a configuration that invalidates existing claims. Neither default changes nor claim reuse SHALL silently reinterpret surviving claim policies. Reclaim policy SHALL govern unavailable-content recovery independently of eviction, source refresh, and useable_app protection.

#### Scenario: Allowed retention override
- **WHEN** an application permits retention counts from three through ten and a claim requests five
- **THEN** the claim records five as its effective retention count and source freshness remains unchanged

#### Scenario: Disallowed override
- **WHEN** a caller requests a policy outside the application's registered allowance
- **THEN** the operation fails before source acquisition, output publication, or claim mutation

#### Scenario: Policy update would invalidate a claim
- **WHEN** a revision-checked policy update disallows a surviving claim's effective policy
- **THEN** it reports `policy_conflict` without changing the registry or claim
