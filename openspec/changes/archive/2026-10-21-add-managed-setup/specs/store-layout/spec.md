# Spec Delta

## MODIFIED Requirements

### Requirement: Read commands do not implicitly initialize a missing store
`saucepan init` SHALL initialize the resolved default or explicit root in basic mode unless `--integrity keyring` is explicitly selected. `--integrity basic` SHALL explicitly select the default. Native creation SHALL provide the same default and explicit choice. Successful initialization SHALL identify the selected mode. Invalid mode values SHALL fail before initialization effects. Except for explicitly invoked setup, other store-backed commands encountering a missing root directory SHALL fail with `unsupported_store`, no success stdout, and guidance to initialize the selected root; they SHALL NOT create it or its signing key. Existing corrupt, legacy or incomplete stores SHALL retain existing error handling without an implicit reset. Help and version SHALL succeed without home lookup or store initialization. Explicit inline fresh operations SHALL remain usable without any store and SHALL not create one as a side effect.

Explicit setup SHALL initialize an absent or empty selected root using the same basic default and initialization reservation contract, or open a supported existing root without changing its integrity mode. It SHALL reject unsupported, corrupt or nonempty uninitialized roots without clearing them. Setup-status SHALL not initialize. An explicitly selected root SHALL never fall back to the user-home root. Existing keyring stores SHALL retain their key requirement without conversion to basic mode.

#### Scenario: First query precedes initialization
- **WHEN** a caller runs saucepan query-sources before the default root exists
- **THEN** the command fails with guidance to run saucepan init
- **AND** the root remains absent

#### Scenario: Initialization followed by querying
- **WHEN** a caller initializes a disposable user-home store and then queries without --root
- **THEN** both commands use the same .saucepan directory

#### Scenario: Default initialization has no native custody prerequisite
- **WHEN** a caller runs `saucepan init` and `saucepan query-sources` without a configured keyring
- **THEN** initialization reports basic mode and the query succeeds subject to existing source visibility
- **AND** neither command provisions or reads a signing key

#### Scenario: Setup initializes explicitly
- **WHEN** setup is explicitly invoked against a missing or empty root
- **THEN** it uses the established initialization contract before preparing managed installation

#### Scenario: Setup encounters a protected or unsupported store
- **WHEN** a protected root cannot access its key or the root uses an unsupported layout
- **THEN** setup reports the error without resetting, converting or replacing existing data
