## Purpose

Define safe removal of installed sauce records and Saucepan-managed Git checkouts.

## Requirements

### Requirement: Uninstall by manifest name
Saucepan SHALL provide `uninstall <manifest-name>` and SHALL remove the matching entry from the local index.

#### Scenario: Uninstall an installed sauce
- **WHEN** a user uninstalls an installed manifest name
- **THEN** Saucepan removes exactly that entry and reports success

#### Scenario: Uninstall an unknown sauce
- **WHEN** a user uninstalls a manifest name that is not installed
- **THEN** Saucepan returns not found with exit code 1 and leaves the index unchanged

### Requirement: Remove only Saucepan-managed checkouts
For GitHub and custom-Git entries, uninstall SHALL remove the checkout only after verifying its computed path is beneath the corresponding managed source directory. For local entries, uninstall MUST NOT delete the stored source path.

#### Scenario: Uninstall a GitHub sauce
- **WHEN** a user uninstalls a GitHub entry whose checkout exists beneath `<root>/github`
- **THEN** Saucepan removes the managed checkout and its index entry

#### Scenario: Uninstall a custom-Git sauce
- **WHEN** a user uninstalls a custom-Git entry whose checkout exists beneath `<root>/customgit`
- **THEN** Saucepan removes the managed checkout and its index entry

#### Scenario: Uninstall a local sauce
- **WHEN** a user uninstalls a local entry
- **THEN** Saucepan removes only the index entry and leaves the stored local path untouched

#### Scenario: Computed path escapes managed root
- **WHEN** a managed entry resolves outside its expected source directory
- **THEN** Saucepan refuses deletion and preserves the index entry

### Requirement: Missing managed checkout is recoverable
Uninstall SHALL succeed in removing an installed GitHub or custom-Git index entry when its expected managed checkout is already absent.

#### Scenario: Checkout was manually removed
- **WHEN** the index contains a managed entry but its checkout directory does not exist
- **THEN** Saucepan removes the stale index entry and reports success
