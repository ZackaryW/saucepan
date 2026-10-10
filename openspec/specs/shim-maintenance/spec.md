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
Current launchers SHALL require a ready application, an active binding, and authenticated ownership matching the launcher and sidecar digests before execution, as defined in [claim-security](../claim-security/spec.md). Readable sidecar configuration alone SHALL NOT authorize launch, establish ownership, or imply compatibility of old binaries with newly written app records. Destructive maintenance SHALL NOT infer ownership solely from filenames or sidecar app UUIDs. Explicit adoption SHALL verify the supplied expected launcher build and matching sidecar before registering ownership. Uninstall encountering a recognizable unregistered pair SHALL require adoption or an explicit choice to leave it unmanaged, and report any files left behind.

#### Scenario: Adopt a copied existing shim
- **WHEN** an existing regular launcher exactly matches the supplied build and its sidecar identifies the selected live app
- **THEN** adoption registers ownership without replacing either file

#### Scenario: Uninstall leaves an unmanaged shim explicitly
- **WHEN** the caller explicitly chooses to leave an unregistered shim during uninstall
- **THEN** the result identifies that leftover and does not describe it as a removed owned launcher
