# Spec Delta

## ADDED Requirements

### Requirement: Setup migrates an unmanaged manager only with explicit verified consent
Ordinary setup SHALL reject an occupied unowned launcher path. Explicit migration SHALL require a caller-selected migration option, expected SHA256 digest and stable operation ID, targeting only the exact self-launcher path in a supported initialized store. Before displacement, Saucepan SHALL verify a regular unlinked raw manager, its expected bytes and bounded version probe, absence of sidecar/owned/reserved conflicts, and that the running setup manager is outside the displaced path. It SHALL create, flush and verify a no-overwrite backup and persist recovery evidence before removing the original path. Rechecking identity under launcher coordination SHALL precede displacement. A digest or version response SHALL not constitute shim ownership or permit moving another file.

Backups SHALL survive success and failure. Links, reparse points, hard links, directories, changed bytes, occupied sidecars and incompatible store layouts SHALL produce a located refusal without overwriting them. Nonempty uninitialized roots SHALL require separate preservation/root preparation rather than automatic conversion. Migration SHALL not modify unrelated store data, credentials, other applications, cache files or shell settings.

#### Scenario: Approved raw-manager migration
- **WHEN** explicit migration identifies the verified raw manager in an initialized root and no ownership conflict exists
- **THEN** its bytes and executable mode are preserved in a reported backup before displacement and normal managed bootstrap publishes the replacement launcher

#### Scenario: Consent omitted
- **WHEN** setup encounters the same raw manager without migration options
- **THEN** it leaves the file unchanged and reports the explicit migration route

#### Scenario: File changes after inspection
- **WHEN** the source or backup no longer matches the expected identity before displacement
- **THEN** migration stops without removing the changed source or claiming a verified backup

#### Scenario: Busy manager on Windows
- **WHEN** an active process prevents displacement
- **THEN** the result reports pending work with the original preserved and does not terminate the process

#### Scenario: Installation fails after displacement
- **WHEN** the raw file has been preserved and removed but bootstrap fails
- **THEN** the backup remains, the result identifies launcher unavailability and retry information, and no restoration overwrites an occupied launcher path

#### Scenario: Unknown sidecar or unsupported root
- **WHEN** an unowned sidecar exists or the root is nonempty and uninitialized
- **THEN** migration refuses automatic displacement and leaves existing contents unchanged
