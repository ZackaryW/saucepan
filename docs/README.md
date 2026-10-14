# Documentation map

The [repository README](../README.md) introduces the current Rust implementation.
The [main OpenSpec specs](../openspec/specs/README.md) define the synchronized
contract before the active changes. The [composition guide](composition.md) and
[policy guide](policy-resources.md) describe the implemented flat contract.
Active change artifacts describe additions awaiting specification synchronization.
[Archived changes](../openspec/changes/archive/README.md) preserve decisions at
their original checkpoints; they are not alternative current contracts.

## Current usage

| Topic | Guide |
| --- | --- |
| Immutable policy definitions, UUID lookup and explicit adoption | [Policy resources](policy-resources.md) |
| Retrieval, copied results, fresh delivery and independent policies | [Composition](composition.md) |
| Default store, anonymous lookup and optional app identity | [Source discovery](source-query.md) |
| Registration, scoped claims, credentials and CLI errors | [Claim CLI](claim-cli.md) |
| Permissions, retention, ensure and recovery | [Claim policies](claim-policies.md) |
| Claim labels, exact scope and context matching | [Claim queries](claim-query.md) |
| Basic checksums and optional native key custody | [Claim integrity](claim-integrity.md) |
| Exact paths, native leases and child processes | [Resource access](resource-access.md) |
| Persistent intent, atomic preparation and group launch | [Registered groups](resource-groups.md) |
| Explicit retention beyond updater process lifetime | [Output pins](output-pins.md) |
| Rust application integration | [Scoped consumer](scoped-consumer.md) |
| Managed install, update, rollback, shims and uninstall | [Application lifecycle](application-lifecycle.md) |
| Platform builds, release ZIPs and publishing | [Releases](releases.md) |
| Utility boundaries and crate inclusion/defer decisions | [Composition utilities](composition-utilities.md#crate-reuse-decisions) |

Store-backed CLI roots default to the user's home `.saucepan`; `--root` explicitly selects an
isolated store. Native `Store` callers still supply a root, and Saucepan shims
forward their installed root. A project-local `.saucepan` is not auto-discovered. Inline temporary/no-store composition does not resolve a home or store.

Anonymous source discovery needs no app UUID, scope or query file, but returns
only explicitly public sources. App-owned claims require an application and an
explicit scope. Credentials are separately opt-in. Source permissions, scope
permissions, public claim exposure and public source exposure are independent.

App integrations keep an `AppSession`; direct store-wide administration is a
separate local trust boundary. App-specific `Store` convenience calls still
enforce policies and cannot supply proof for protected apps. JSON payloads are
readable and verified using basic checksums by default, or HMAC with explicitly
selected native key custody. Existing current-format protected roots stay protected;
earlier layouts are not migrated and old-reader compatibility is not promised.

## Verification evidence

[Group/pin verification](resource-group-verification.md) records current Windows
checks and the retained consumer fixtures. [Flat-policy verification](policy-verification.md)
preserves the earlier two-scope Git consumer evidence. Run:

```powershell
cargo test --locked --test policy_acceptance
cargo test --locked --all-targets
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
openspec validate --all --strict
```

The optional native credential-manager checks require separate selection. Basic
integrity and injected-key tests do not establish OS keyring availability.
Linux/macOS execution and the minimum Rust version require their own validation.

[Composition](composition-verification.md), [optional keyring](optional-keyring-verification.md),
[claim registry](claim-verification.md), [mock acceptance](mock-acceptance.md),
[resource access](resource-access-verification.md) and [lifecycle](lifecycle-verification.md)
reports preserve dated evidence from earlier contracts. Their earlier JSON
payloads and Python acceptance harnesses are not current flat-contract recipes.
Do not run them against a current store or reopen preserved historical mock data
with the current manager. No migration or compatibility decoder is provided.
