## ADDED Requirements

### Requirement: Linux x86_64 musl release binary
Saucepan's release workflow SHALL build and publish a statically-linked `x86_64-unknown-linux-musl` binary as a release asset for every tagged release, alongside the existing macOS and Windows assets.

#### Scenario: Tagged release produces a Linux asset
- **WHEN** a `v*` tag is pushed (or the release workflow is dispatched for a tag)
- **THEN** the published release includes a `saucepan-x86_64-unknown-linux-musl` asset

#### Scenario: Existing macOS and Windows assets are unaffected
- **WHEN** the Linux build job is added to the release workflow
- **THEN** the existing macOS (x86_64, aarch64, universal) and Windows (x86_64, aarch64, i686) build jobs and asset names remain unchanged
