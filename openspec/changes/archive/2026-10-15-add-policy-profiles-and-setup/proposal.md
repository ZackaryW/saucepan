# Proposal

## Why

PowerSpec, Zush and power-eagle had to assemble the same policy choices and setup steps independently. Optional named profiles can make those choices discoverable and repeatable without replacing Saucepan's flat UUID policy authority, while explicit SDK lifecycle calls can expose the installation machinery Rust already owns.

## What Changes

- Add `managed`, `explicit-update` and `temporary` built-in profiles as immutable, inspectable recipes that expand to ordinary policy definitions and operation-specific UUID references. `managed` is the sole default profile when profile expansion is requested without a name; existing operations with no profile remain unchanged.
- Keep retrieval freshness, acquisition updates, revision history, destination ownership, optional verification, app identity and source/scope restrictions separate. Profiles do not enroll apps, create pins, grant access or install executables implicitly.
- Provide root-free profile preview and explicit registration, typed UUID override composition, and read-only effective-policy explanation using the existing evaluator. Changed definitions receive new UUIDs; existing associations require explicit adoption.
- Integrate expanded policies with existing consumer registration/claims and managed installation/bootstrap. Setup adds `useable_app_policy` explicitly, preserves current probes, receipts and activation boundaries, and rejects temporary delivery for stable executable installation.
- Add Python/TypeScript profile and bounded managed lifecycle adapters, including truthful pending-operation results. SDK installation/construction still does not install the manager; first setup starts with an explicitly obtained manager and shim.
- Document first setup and update flows as user instructions linked from README; policy decisions and acceptance requirements live here.

## Capabilities

### New Capabilities
- `policy-profiles`: Pure named recipe expansion, immutable identities, target-specific references and explicit customization/registration.

### Modified Capabilities
- `policy-resources`: Authorized read-only effective-policy explanations, preserving provenance and existing precedence.
- `application-lifecycle`: Profile-assisted managed setup through existing installation operations.
- `self-management`: Profile-assisted Saucepan release bootstrap and explicit first-setup instructions.
- `language-sdks`: Typed profile/explanation and managed lifecycle operations with explicit setup and pending results.

## Impact

Changes target `src/models/policy`, `src/core/policy`, `src/cli/policies.rs`, lifecycle/bootstrap adapters, both SDKs and `sdk/contracts`. Reuse existing policy registration, adoption, immutable output history, lifecycle journaling, probes and shims. No new crate is expected, no schema-version inheritance or legacy adapters are introduced, and existing no-profile behavior is preserved. Consumer migrations, independent retention from existing pins, transport extraction, automatic recovery/retries, background updates, PATH mutation, binary downloads by SDK installation and hosted CI dispatch are outside this change.
