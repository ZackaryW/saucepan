## MODIFIED Requirements

### Requirement: Index precedence is deterministic
When more than one registered index describes the same target, Saucepan SHALL resolve using registration order and SHALL emit a warning on stderr during every resolution that encounters a later matching entry. The warning SHALL identify the shadowed and winning indexes.

#### Scenario: Two indexes describe one target
- **WHEN** two registered indexes both describe the same target with different manifests
- **THEN** the earlier-registered index supplies the manifest and Saucepan warns on stderr that the later entry was shadowed

#### Scenario: Collision is reported on a later resolution
- **WHEN** the same colliding registrations are consulted by a subsequent install or update resolution
- **THEN** Saucepan emits the shadowing warning again rather than treating an earlier warning as persistent state
