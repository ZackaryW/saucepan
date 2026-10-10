# Documentation map

The [repository README](../README.md) introduces the current Rust implementation.
The [main OpenSpec specs](../openspec/specs/README.md) define its behavior.
[Archived changes](../openspec/changes/archive/README.md) preserve decisions at
their original checkpoints; they are not alternative current contracts.

## Current usage

| Topic | Guide |
| --- | --- |
| Default store, anonymous lookup and optional app identity | [Source discovery](source-query.md) |
| Registration, scoped claims, credentials and CLI errors | [Claim CLI](claim-cli.md) |
| Permissions, retention, ensure and recovery | [Claim policies](claim-policies.md) |
| Claim labels, exact scope and context matching | [Claim queries](claim-query.md) |
| Basic checksums and optional native key custody | [Claim integrity](claim-integrity.md) |
| Exact paths, native leases and child processes | [Resource access](resource-access.md) |
| Rust application integration | [Scoped consumer](scoped-consumer.md) |
| Managed install, update, rollback, shims and uninstall | [Application lifecycle](application-lifecycle.md) |
| Platform builds, release ZIPs and publishing | [Releases](releases.md) |

CLI roots default to the user's home `.saucepan`; `--root` explicitly selects an
isolated store. Native `Store` callers still supply a root, and Saucepan shims
forward their installed root. A project-local `.saucepan` is not auto-discovered.

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

[Claim registry verification](claim-verification.md) records a Windows x86_64
checkpoint using the current authenticated layout. Its counts and binary digests
belong to that dated run, before the later source-query CLI changes. Relevant
current checks are:

```powershell
cargo test --offline --test claim_acceptance
cargo test --offline --test source_query_cli --test query_cli
cargo fmt --check
cargo clippy --offline --all-targets -- -D warnings
openspec validate --all --strict
```

The integrated claim fixture uses real built binaries and a fresh temporary root,
with local provider inputs. Release discovery and ZIP selection are exercised
separately by `tests/remote.rs`. The older complete two-build release/bootstrap
recipe predates authenticated registries; its report is not proof that the full
release-to-self-update flow has been rerun against the current contract. That
combined acceptance flow still needs current evidence. Linux/macOS runtime
behavior and Rust 1.89 execution also remain unverified here.

[Mock acceptance](mock-acceptance.md), [resource-access verification](resource-access-verification.md)
and [lifecycle verification](lifecycle-verification.md) are historical reports.
Their binary identities, counts, Python harness commands and earlier layouts
describe the recorded checkpoints. The Python scripts are development harnesses,
not runtime dependencies. Do not use their old-layout recipes to modify a current
store or reopen the preserved `mock/` with the current manager.

On 2026-10-10 the completed changes were synchronized and archived; strict
validation passed for all 18 main specs. Documentation reconciliation does not
constitute a new full acceptance or cross-platform verification run. Focused
checks performed while reconciling documentation are recorded below separately
from the historical reports.

The 2026-10-10 reconciliation checks passed on Windows: 14 test entry points
across `query_cli`, `source_query_cli`, `registry_security`, `cli`, and the selected
protected-shim lifecycle test. The source-query count includes its child-fixture
entry point. Documentation checks also validated local links, JSON examples and
all 18 main specs. The full suite and historical Python acceptance harnesses were
not rerun for these documentation edits.
