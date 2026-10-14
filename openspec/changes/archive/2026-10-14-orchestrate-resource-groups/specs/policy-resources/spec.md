# Spec Delta

## ADDED Requirements

### Requirement: Persistent group policy roles remain distinct and restrictive
Persistent group definitions SHALL reference registered immutable policies by UUID. Group-level references SHALL accept only access restrictions and path-verification floors; acquisition behavior SHALL use each member's existing typed policy references and normal source/app defaults and constraints. Preparation SHALL intersect group restrictions with current authority for each action it actually requires, including acquire, update or recover; opening SHALL evaluate lease. A lease-only boundary SHALL NOT authorize preparation. Missing scope witnesses SHALL fail scope-restricted boundaries. New definitions SHALL require new policy UUIDs and explicit association adoption. Group replacement SHALL NOT silently adopt policies or replace context on an existing claim. Portable caller catalogs SHALL remain available for inline leases but SHALL NOT become unregistered authorities for persisted groups.

#### Scenario: Lease boundary does not permit fetching
- **WHEN** a group allows lease but needs an acquire action to prepare missing content
- **THEN** preparation is denied before provider work

#### Scenario: Member behavior does not become group authority
- **WHEN** a member's permissive behavior references conflict with current app source restrictions
- **THEN** preparation is denied and no group reference broadens the app's authority

#### Scenario: Existing claim has another adopted behavior
- **WHEN** group replacement supplies different update or revision behavior for a matching existing claim
- **THEN** preparation requires explicit permitted claim adoption instead of mutating it through group registration

### Requirement: Durable pin policy is an explicit separate retention role
A pin request SHALL reference exactly one registered immutable pin_policy with retention until_released. It SHALL reject other policy roles, unknown fields and conflicting references. Pin policy SHALL not be inherited through source/application defaults or ordinary claim/group references. Ordinary acquisition and inline group admission SHALL reject pin policy in their existing roles. Group preparation SHALL opt in through a separate pin request, not an embedded policy definition. Current authority SHALL support distinct pin and unpin actions; selecting pin policy SHALL NOT grant either permission or disable required verification. Removing a policy attachment or changing current permissions SHALL not retroactively release already committed pins.

#### Scenario: Pin behavior is not silently inherited
- **WHEN** a caller prepares a group without a pin request
- **THEN** no durable pin is created even when the catalog contains pin_policy definitions

#### Scenario: A lease-only caller requests durable retention
- **WHEN** an app allowed lease but denied pin attempts pin creation
- **THEN** the operation fails despite a valid pin policy reference
