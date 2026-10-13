## MODIFIED Requirements

### Requirement: The built application completes the simulated installation flow
Acceptance verification SHALL use the actual built Saucepan executable and built shim with an explicit isolated simulated root. The initial reimplementation used `mock/` directly; current regression runs SHALL preserve that historical installation and use a fresh basic-mode root under the current explicit integrity contract, as defined in [lifecycle-recovery](../lifecycle-recovery/spec.md). The built application SHALL be packaged and acquired through the release provider using a controlled release fixture, indexed, claimed, activated with `useable_app`, and launched through the stable shim. Manually authored success records or stand-in executables SHALL NOT substitute for this flow. Test endpoint selection SHALL be explicit and SHALL NOT change the production provider default. Historical release-flow results SHALL NOT by themselves establish that this combined flow has been verified against a later store contract. Basic acceptance SHALL complete without a keyring service. A separately selected protected manager/shim integration SHALL record native custody results; successful basic acceptance SHALL NOT be represented as verification of native keyring support.

#### Scenario: A local build is acquired and launched as a managed release
- **WHEN** a fresh-root recipe runs simulated bootstrap against a loopback release fixture serving the packaged local build
- **THEN** that root's `store/<entry-uuid>/` contains executable bytes matching the build's digest, its `index/` and `claims/` record the output, application binding and claim, and its stable shim launches that selected output
- **AND** verification records the resulting identities and preserves the historical `mock/` installation

#### Scenario: The simulated installation updates between real builds
- **WHEN** a second distinguishable build is acquired and activated through the same simulated flow
- **THEN** the shim launches the newly selected build, a failed candidate leaves the previous binding usable, and eviction respects active protection
- **AND** verification records actual build identities and outputs rather than inferring a version change from release tags alone
