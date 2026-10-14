## ADDED Requirements

### Requirement: Group consumers receive named paths with shared lifetime protection
Native access SHALL expose the complete admitted group through one lifetime-bearing handle. The CLI SHALL provide `run-group FILE [--policies CATALOG] -- PROGRAM ARG...` using the existing default/explicit root and optional app/credential context. It SHALL admit the entire group before launching, forward literal arguments and inherited streams, preserve the working directory, and propagate child exit status and supported interruption behavior. It SHALL set SAUCEPAN_RESOURCE_GROUP_FILE to a bounded UTF-8 JSON descriptor whose members are keyed by request name and identify exact output UUIDs, optional claim UUIDs, selected directories, relative views and verification. The descriptor SHALL contain no secrets and SHALL NOT be accepted as a lease or authorization token. Group runs SHALL remove inherited singular resource/source variables from the child's environment without modifying the parent environment.

#### Scenario: A command uses hook and service repositories
- **WHEN** a child opens its group descriptor and reads command, hooks and services from three outputs
- **THEN** all three paths remain protected during the supported child lifetime without app claims being required for output-only groups
- **AND** no success JSON is mixed into the child's stdout

#### Scenario: Literal arguments and child errors survive forwarding
- **WHEN** a group consumer receives spaces or shell metacharacters in arguments and exits nonzero
- **THEN** arguments arrive unchanged and the CLI forwards the child's exit status without wrapping it as admission success

### Requirement: Group execution preserves every inherited resource lifetime
Dedicated group execution SHALL establish native ownership for every distinct output and the descriptor before starting the child. Supported children retaining inherited handles SHALL retain all resources and the descriptor after unexpected wrapper exit. Native in-process group guards SHALL remain non-inheritable by default. Preparation or launch failure SHALL leave no live orphan protection; closed markers and owned scratch SHALL be eligible for existing native-evidence-based cleanup. No PID-only or elapsed-time rule SHALL revoke protection. Ordinary native group access SHALL NOT create per-use process markers.

#### Scenario: Wrapper exits while several outputs are in use
- **WHEN** the wrapper terminates while a supported consumer retains the inherited handles and subsequently reads its descriptor
- **THEN** the descriptor and every group output remain available and reclaim defers their deletion until their protected lifetimes end

#### Scenario: A later inheritance step or spawn fails
- **WHEN** group setup fails after some native handles have been prepared
- **THEN** no child begins resource work and all attempt-owned handles are released so cleanup can remove closed markers and scratch

#### Scenario: Native group is unrelated to a spawned process
- **WHEN** a native caller holds a group and launches an unrelated child outside the dedicated execution boundary
- **THEN** that child does not inherit the group's protection and dropping the native group releases its guards
