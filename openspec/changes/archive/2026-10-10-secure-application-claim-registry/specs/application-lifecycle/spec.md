# Spec Delta

## ADDED Requirements

### Requirement: Application lifecycle coordinates the claim registry
Install, managed update, rollback, activation, context changes, release, and uninstall SHALL use the separate authoritative claim registry and the same application revision boundary. Applicable app credentials SHALL be checked before preparation or mutation. Successful transitions SHALL publish registry members, policy state, claim selection, application metadata and bindings as one recoverable generation. Failed or stale transitions SHALL NOT recreate removed registries or restore old context/policies. Uninstall SHALL revoke app access and retire its live registry with the live app record while retaining appropriately protected receipts. Existing resource leases SHALL survive registry removal.

#### Scenario: Crash separates member and application publication
- **WHEN** a committed managed update is interrupted between HMAC-protected claim and app publication
- **THEN** recovery completes one coherent claim and binding generation before subsequent access

#### Scenario: Uninstall overtakes ensure
- **WHEN** uninstall commits while an earlier ensure prepares content
- **THEN** ensure cannot republish the application or its claim registry

#### Scenario: Live consumer survives registry removal
- **WHEN** uninstall retires the registry while a native consumer owns a lease
- **THEN** the consumer's output remains protected until its native lifetime ends
