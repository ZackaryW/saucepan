# Spec Delta

## ADDED Requirements

### Requirement: Explicit self-release changes commit request and activation together
An explicit self-update tag change SHALL be a single managed transition under the application's existing revision and authorization fences. The replacement request SHALL differ only in its release selector; its source, application scope, executable selection and adopted policies SHALL remain unchanged. The selected request, claim output and active binding SHALL publish as one recoverable generation after integrity verification and the bounded usability probe. Retry fingerprints SHALL include the requested tag. Intervening application mutation SHALL conflict rather than overwrite newer intent. Existing update-without-selector and rollback contracts SHALL remain unchanged.

#### Scenario: Crash during explicit tag update publication
- **WHEN** a tag-selecting self-update is interrupted after commit intent
- **THEN** recovery exposes a coherent old or new request, claim and binding rather than a mixed generation

#### Scenario: Concurrent deactivation
- **WHEN** another operation deactivates the application after the tag update observed its revision
- **THEN** the tag update reports conflict and does not restore an active binding

#### Scenario: Retry changes target release
- **WHEN** an existing self-update operation ID is supplied with another tag
- **THEN** the retry is rejected without acquiring or activating that other release
