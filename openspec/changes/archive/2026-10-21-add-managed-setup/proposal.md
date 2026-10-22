# Proposal

## Why

Saucepan has a managed bootstrap lifecycle, but first installation still requires callers to assemble policy, source, release, and shim inputs manually. Powerspec consequently maintains a raw manager at the intended managed-launcher path; an official setup entry point will establish one installation owner and a supported transition for that existing layout.

## What Changes

- Add a Rust-owned `setup` command and read-only `setup-status` command, using the existing bootstrap, policy, release-provider, ownership, and recovery APIs.
- Supply thin release-pinned `install.ps1` and `install.sh` launchers that verify and stage the matching manager/shim outside the store, then invoke setup. SDK installation and import remain side-effect-free.
- Make repeated setup reuse a verified installation without upgrading it. Persist preparation identities and report actual setup/lifecycle outcomes; explicit self-update remains the update owner.
- Allow explicit self-update to select an exact replacement release tag, committing that request with the validated claim/binding so a pinned first install remains deliberately updatable.
- Add explicit, digest-bound migration of an unmanaged raw manager occupying the launcher path in a supported initialized store. Preserve a verified backup and recoverable progress; ordinary setup never infers file ownership. The user expressly selected this migration behavior.
- Publish typed setup/status APIs in both SDKs, update release validation and documentation, and verify real manager/shim flows in isolated roots.
- **Behavior change:** authorized setup can initialize a missing/empty store; read commands still cannot. Existing low-level init/bootstrap/self-update contracts remain available.

## Capabilities

### New Capabilities

None; extend the existing installation capabilities.

### Modified Capabilities

- `self-management`: Convenient setup, explicit release selection, repeat invocation, machine-readable installation inspection, and consumer handoff.
- `store-layout`: Explicit setup initialization exception, preserving integrity modes and rejection of unsupported roots.
- `lifecycle-recovery`: Durable setup preparation identities and recovery linked to the existing installation operation.
- `application-lifecycle`: Atomic request/claim/binding publication for an explicitly selected self-update release.
- `shim-maintenance`: Opt-in preservation and displacement of a verified unmanaged manager without implicit adoption.
- `language-sdks`: Thin typed setup/status methods with explicit manager/shim inputs and truthful result handling.
- `sdk-distribution`: Verified installer script assets from the release commit, without SDK installation hooks.

## Impact

Baseline: Saucepan checkout `6a4cd06`, Cargo version `0.10.0`. Affects CLI routing, a setup feature module, lifecycle/metadata integration, both SDKs, release scripts/workflows, focused tests and first-install documentation. Reuse current utilities and dependencies; no package-manager publication, shell-profile modification, automatic store-format migration, or broad installer framework is planned.

The dependent consumer change is `../../../../../powerspec/openspec/changes/adopt-saucepan-managed-setup/` from this directory. Saucepan owns this repository's implementation; that change owns Powerspec adoption. No production store, installed binary, release tag, or application code is changed by these planning artifacts. A new release containing setup is required; existing v0.10.0 binaries do not gain this command by updating the SDK.
