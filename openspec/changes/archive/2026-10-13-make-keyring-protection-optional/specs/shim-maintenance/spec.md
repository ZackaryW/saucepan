## MODIFIED Requirements

### Requirement: Existing unregistered shims require explicit adoption or exclusion
Current launchers SHALL require a ready application, an active binding, and ownership records verified under the store's selected integrity mode and matching the launcher and sidecar digests before execution, as defined in [claim-security](../claim-security/spec.md). Launchers SHALL open basic stores without keyring access and SHALL NOT downgrade protected stores when custody is unavailable. Readable sidecar configuration alone SHALL NOT authorize launch, establish ownership, or imply compatibility of old binaries with newly written app records. Destructive maintenance SHALL NOT infer ownership solely from filenames or sidecar app UUIDs. Explicit adoption SHALL verify the supplied expected launcher build and matching sidecar before registering ownership. Uninstall encountering a recognizable unregistered pair SHALL require adoption or an explicit choice to leave it unmanaged, and report any files left behind.

#### Scenario: Adopt a copied existing shim
- **WHEN** an existing regular launcher exactly matches the supplied build and its sidecar identifies the selected live app
- **THEN** adoption registers ownership without replacing either file

#### Scenario: Uninstall leaves an unmanaged shim explicitly
- **WHEN** the caller explicitly chooses to leave an unregistered shim during uninstall
- **THEN** the result identifies that leftover and does not describe it as a removed owned launcher
