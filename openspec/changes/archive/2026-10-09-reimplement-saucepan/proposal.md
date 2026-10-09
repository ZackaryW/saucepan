# Proposal

## Why

Saucepan needs a clean reimplementation whose source discovery, acquired outputs, and application claims have independent lifecycles. The previous schema couples source snapshots and application bookkeeping in ways that do not fit release assets, per-item policies, or Saucepan managing its own executable.

## What Changes

- **BREAKING**: Build a new Rust core and CLI in this repository, with a newly defined JSON contract. Do not import the previous format version, deserialize legacy records, migrate legacy stores, or preserve old CLI/SDK schemas.
- Separate `.saucepan/source/` definitions and provider working data, `.saucepan/index/<uuid>/index.json` metadata, and `.saucepan/store/<entry-uuid>/` acquired content. Store content retains its natural structure; `items/` is not a required directory.
- Add `github_release` discovery, ordered platform/asset filters, binary selection, and a 24-hour source-check cache. A release source does not clone its repository.
- Retain application claims linking the acquisition request to a specific output entry. Multiple versions coexist; ordinary claims do not prevent reclaim.
- Make identical-pull handling and output eviction explicit policies, accepting item-specific overrides in acquisition payloads. The default retains the latest ten output entries.
- Introduce the named `useable_app` policy and a stable executable shim so Saucepan can acquire, activate, and retain a usable release of itself without replacing a running cached binary.
- Use direct UUID/path lookup and typed records rather than a general component graph engine. Retain ordered selection policies without carrying forward the abandoned flat graph representation.
- Follow the supplied `src/` ownership structure: domain models in `models/`, workflows in matching `core/` areas, adapters in `cli/`, and portable mechanics in `utils/`. Plan and verify required utilities before their application integration.
- After implementation, exercise the built Saucepan in repository-local `mock/` as a simulated `.saucepan`, including release acquisition into `mock/store/`, app claims, activation, and launch through `mock/bin/saucepan[.exe]`.

## Capabilities

### New Capabilities

- `store-layout`: Independent source, index, and content records; publication and recovery boundaries.
- `source-resolution`: Source providers, platform/asset selection, exact revisions, and source refresh caching.
- `output-acquisition`: Acquisition requests, immutable outputs, policy-driven reuse, and provenance.
- `application-claims`: App registration, request-bearing claims, updates, and reclaimed-entry visibility.
- `output-policies`: Item-level policy overrides, default latest-ten retention, and output reclaim.
- `self-management`: `useable_app`, stable shims, activation, and Saucepan's release-only bootstrap.

### Modified Capabilities

None. This repository has no existing specs. The sibling `saucepan` repository is behavioral reference material, not an inherited specification or migration target.

## Impact

Introduces the Rust package, core models, filesystem persistence, providers, policy evaluation, CLI, shim, and focused tests within the supplied source placeholders. Existing integrations must adopt the new acquisition protocol; SDK ports are separate follow-up work. Planning records proposed defaults for retention scope and activation so they are reviewable before implementation. `mock/` is the inspectable acceptance target after implementation, using the actual built application rather than a stand-in. No source changes are made in the sibling repository and no production store is rewritten as part of this change's planning.
