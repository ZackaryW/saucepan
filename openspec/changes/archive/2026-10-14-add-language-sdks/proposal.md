# Proposal

## Why

Power Eagle, Zush and PowerSpec need the current Saucepan resource APIs from TypeScript or Python. Independent subprocess adapters would duplicate payload handling and easily confuse paths with retention, optional identity with mandatory registration, or child exit statuses with Saucepan errors.

## What Changes

- Add thin installable SDKs at `sdk/python/` and `sdk/ts/`, with typed operations for initialization, source discovery/registration, policy lookup/adoption, optional application context, claims, retrieval/acquisition, registered groups, pins and guarded child execution.
- Accept ordinary request objects instead of requiring callers to create JSON files. Keep Rust CLI payloads and registered policy UUIDs authoritative, with no legacy payload decoder, native-language binding or duplicated storage/policy engine.
- Make client construction inert. Allow an explicit binary and root, otherwise use the executable on PATH and delegate the default store to Rust. Never implicitly register apps, enable verification/credentials, pin content, fetch a binary or compile Rust.
- Distinguish JSON operations, bare path inspection, explicit durable retention and child-process execution. Preserve current authority, expected revisions, idempotency keys and indeterminate-outcome handling.
- Support Python local/editable and Git-subdirectory installation; TypeScript local source builds and installable npm tarballs containing JavaScript and declarations. Build Python wheel/sdist and TypeScript tarball release assets without publishing to package registries.
- Add a shared contract inventory, real-binary tests and consumer-shaped examples. Integrate cost-controlled manual/release SDK checks and exact release-asset validation.

## Capabilities

### New Capabilities

- `language-sdks`: Typed Python/TypeScript access to the existing CLI, optional context, transport failures and explicit lifetime boundaries.
- `sdk-distribution`: Source installation, complete SDK packages, release artifacts and cross-language contract verification.

### Modified Capabilities

None. Existing Rust source, claim, policy, group, pin and execution requirements remain authoritative; the SDKs add access paths to those capabilities.

## Impact

New packages and fixtures under `sdk/`; root documentation, `scripts/release.py`, its tests, `.github/workflows/release.yml`, and narrowly scoped SDK check workflow wiring. Rust remains the only manager implementation and gains no Python/Node runtime dependency. Builds may add development/build dependencies for SDK packaging and types, with decisions recorded in the design.

This builds on implemented `orchestrate-resource-groups` (24/24 tasks, still unarchived); use its current models and delta specs until synchronization. Do not archive or modify that change as part of this work. No consumer repository changes, legacy SDK migration, registry publishing, hosted workflow execution or binary auto-installation are included.
