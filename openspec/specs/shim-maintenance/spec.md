# shim-maintenance Specification

## Purpose

Make launcher installation, adoption, replacement and removal depend on explicit ownership and recoverable file operations rather than filename guesses.

## Requirements

### Requirement: Shim ownership reserves both launcher and sidecar paths
Shim installation SHALL record its owning application and expected identities of both launcher and configuration files. It SHALL reject name collisions with owned or reserved paths and occupied unowned files, including case and derived-sidecar aliases. It SHALL reject links, reparse points and non-regular destinations without modifying them.

#### Scenario: Two applications request colliding launcher names
- **WHEN** concurrent requests use names whose executable or derived configuration paths collide
- **THEN** at most one reserves those paths and the other reports a conflict without overwriting files

#### Scenario: Only the configuration path is occupied
- **WHEN** an executable destination is absent but its sidecar belongs to another file or operation
- **THEN** installation fails preflight without replacing that sidecar

### Requirement: Launcher replacement preserves ownership and application selection
Explicit replacement SHALL operate on an owned shim, preserve its application/configuration, and leave claims and binding unchanged. Routine app update SHALL NOT replace the launcher. Failed or interrupted replacement SHALL preserve a working old pair or a recoverable recorded transition; it SHALL NOT silently repoint the launcher or publish unverified bytes.

#### Scenario: Replacement encounters a running launcher
- **WHEN** native coordination or filesystem sharing prevents replacing the launcher
- **THEN** the operation reports pending work while preserving the old usable pair and unrelated applications remain available

#### Scenario: Existing launcher has been externally changed
- **WHEN** the present executable or configuration differs from its recorded identity
- **THEN** replacement retains `commit_intent` with a file-identity conflict reason and leaves the changed files untouched for operator resolution before retry

### Requirement: Removal is limited to verified owned files
Uninstall cleanup SHALL delete only expected owned regular files after rechecking identity. Missing files SHALL count as already cleaned. Busy files SHALL remain pending; changed or unknown files SHALL be preserved and reported. A live target's output protection SHALL be independent of launcher cleanup.

#### Scenario: Configuration changes between removal attempts
- **WHEN** an owned sidecar is replaced with different contents before retry
- **THEN** retry retains `cleanup_pending` with a file-identity conflict reason without deleting or claiming successful cleanup of that sidecar

#### Scenario: Target remains alive after shim removal
- **WHEN** the launcher can be removed while its target still owns an output lease
- **THEN** ordinary reclaim continues to defer that output until the final lease ends

### Requirement: Existing unregistered shims require explicit adoption or exclusion
Current launchers SHALL require a ready application, an active binding, and ownership records verified under the store's selected integrity mode and matching the launcher and sidecar digests before execution, as defined in [claim-security](../claim-security/spec.md). Launchers SHALL open basic stores without keyring access and SHALL NOT downgrade protected stores when custody is unavailable. Readable sidecar configuration alone SHALL NOT authorize launch, establish ownership, or imply compatibility of old binaries with newly written app records. Destructive maintenance SHALL NOT infer ownership solely from filenames or sidecar app UUIDs. Explicit adoption SHALL verify the supplied expected launcher build and matching sidecar before registering ownership. Uninstall encountering a recognizable unregistered pair SHALL require adoption or an explicit choice to leave it unmanaged, and report any files left behind.

#### Scenario: Adopt a copied existing shim
- **WHEN** an existing regular launcher exactly matches the supplied build and its sidecar identifies the selected live app
- **THEN** adoption registers ownership without replacing either file

#### Scenario: Uninstall leaves an unmanaged shim explicitly
- **WHEN** the caller explicitly chooses to leave an unregistered shim during uninstall
- **THEN** the result identifies that leftover and does not describe it as a removed owned launcher

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
