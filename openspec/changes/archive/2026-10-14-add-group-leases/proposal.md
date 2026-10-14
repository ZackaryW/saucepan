# Proposal

## Why

Consumers such as Zush execute a command together with hooks and services from several acquired repositories. Saucepan currently protects one selected output per lease/CLI execution call, leaving consumers to assemble coordinated admission, narrowing access boundaries and complete lifetime protection themselves.

## What Changes

- Add a non-persistent group lease over named existing output or claim references in one store, with optional contained directory views.
- Admit all members together under one caller identity and coordinated metadata selection; return no partial group and release acquired protection on failure.
- Allow optional flat policy references to narrow the group's source/action/scope access and strengthen path verification. Group boundaries cannot grant authority or change app/source associations.
- Retain one protection per distinct output while exposing every requested named view; preserve selected output identities through subsequent claim updates, release and policy changes.
- Add native guarded access and a `run-group` child workflow with a named resource descriptor whose lifetime follows the consumer.
- Keep unrestricted local output access, optional app identity and optional credentials. No synthetic claims are created.

## Capabilities

### New Capabilities

- `group-leases`: Bounded named membership, optional caller context, coordinated admission, failure cleanup and stable multi-output lifetime.

### Modified Capabilities

- `policy-resources`: Allow group-only restrictive use of referenced access definitions and path-verification requirements without granting request authority.
- `resource-access`: Expose grouped native access and literal child execution with a named resource map and inherited protection for every member.

## Impact

Add focused `models/lease` and `core/lease` modules, reuse `core/policy`, existing store leases, lifetime markers and process-launch boundaries, and extend CLI dispatch and consumer documentation. Existing single-resource operations retain their contracts. No new crate or persistent store schema is planned. Use isolated Rust/subprocess fixtures shaped like Zush; do not edit Zush or Power Eagle.

## Non-goals

Persistent group registration or aliases, automatic retrieval/recovery, atomic multi-resource updates, cross-store or multi-application groups, arbitrary paths or serialized temporary handles as group members, OS sandboxing, retroactive revocation of live leases, and compatibility adapters are excluded. Persistent registration, retrieval orchestration and coordinated updates belong to a separate change after this one is complete.
