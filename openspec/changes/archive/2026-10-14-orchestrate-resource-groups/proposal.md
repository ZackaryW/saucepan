# Proposal

## Why

Consumers can now retain several prepared resources together, but must remember the members, prepare each one and coordinate updates themselves. A failure midway through several independent claim updates can leave a consumer's resource set on mixed selections.

In-process hosts such as Power Eagle also need exact output paths to survive beyond a short CLI process. Explicit durable pins let them use immutable store entries directly without copying packages into a second managed destination.

## What Changes

- Register bounded named resource groups by UUID and an explicit owner-local registration key, with revision-checked replacement and removal. Local groups need no app or credentials; application groups use one explicit application context.
- Save existing resource targets or managed acquisition intents, preserving the retrieve/source-material and acquire/copied-output distinction and flat policy UUID references.
- Add explicit group preparation that evaluates current permissions and each member's update/reclaim policies, retrieves required material and prepares copied results before publishing selections.
- Publish a complete selected generation and any participating claim changes through one recoverable metadata transaction. Failed preparation leaves the previous generation and claims unchanged; fetched cache material may remain reusable.
- Open or run a registered prepared group through the existing lifetime-bearing lease machinery. Opening remains read-only; a separately named prepare-and-run convenience explicitly opts into preparation.
- Add independently owned, explicitly released pins for exact managed outputs or a selected group generation. Pins survive process exit and selection updates, prevent reclaim and never make payloads mutable. A changed payload receives a new output UUID.
- Allow preparation to publish an opted-in pin with the selected generation in the same transaction. Previous pins stay effective until their owners release them, so existing plugin instances can continue reading their original versions.
- Keep existing inline group leases and independently optional claims, credentials, verification and caching flows. Persistent groups in this change coordinate managed outputs; temporary/no-store and caller-owned single-resource APIs remain independent.

## Capabilities

### New Capabilities

- `resource-groups`: Persistent group definitions, explicit preparation, atomic selected generations, lookup and retained consumption.
- `output-pins`: Durable consumer-owned retention of exact output sets, explicit release, inspection and protected publication handoff.

### Modified Capabilities

- `application-claims`: Coordinated publication of multiple participating claim selections without duplicate ownership or implicit release.
- `policy-resources`: Separate restrictive preparation/lease boundaries and per-member behavior references for registered groups.
- `output-policies`: Explicit pins override reclaim eligibility independently of ordinary claim history, live leases and active bindings.
- `store-layout`: Verified group/pin records and recoverable atomic generation/pin publication.

## Impact

Add `models/group`, `core/group`, `models/pin` and `core/pin` beside the existing lease modules. Integrate claim preparation/publication, metadata catalog identities, policy fences, reclaim, managed acquisition, CLI commands and documentation. Reuse existing native guards and metadata transactions; do not add an IAM engine, daemon, Python binding, database or compatibility layer. Destination synchronization, runtime copy claims, lifecycle hooks, consumer code changes, background scheduling, dependency graphs, multi-store transactions and atomic external hook/service execution are outside this change.
