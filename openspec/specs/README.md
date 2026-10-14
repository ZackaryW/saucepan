# Specification map

These main specs describe the synchronized Saucepan baseline. [Archived changes](../changes/archive/README.md)
record historical decisions and are not alternative versions to inherit at runtime.
Adding a provider or policy type does not establish backward schema compatibility.

The synchronized baseline includes integrity-verified application claim registries,
explicit scopes, covered ensure, optional credentials, registered recovery and
consumer error codes. Source discovery supports anonymous public lookup, optional
application identity and direct CLI filters, with the user-home `.saucepan` store
as the default. New stores use basic checksums without keyring access; explicit
keyring mode retains HMAC protection. App credentials remain independently opt-in
in both modes. The [repository README](../../README.md) organizes user-facing
guides by topic. Designs, dependency decisions and dated verification reports
belong to their active or archived OpenSpec change, not to a parallel docs index.

This implementation accepts its explicit basic contract and supported protected
contract; protected stores never fall back to basic verification. Historical
references to inline claims or older record defaults do not imply a decoder or
migration path for those stores.

Read overlapping requirements together using these responsibility boundaries:

| Capability | Owns |
| --- | --- |
| [store-layout](store-layout/spec.md) | Root layout, current data contract and rejection of pre-reimplementation stores |
| [source-resolution](source-resolution/spec.md) | Provider resolution, discovery freshness and asset selection |
| [source-query](source-query/spec.md) | Source metadata discovery, public visibility and optional CLI filters |
| [source-retrieval](source-retrieval/spec.md) | Original source material, retrieval modes and lifetime |
| [output-acquisition](output-acquisition/spec.md) | Immutable output identity, acquisition and reuse |
| [acquisition-refresh](acquisition-refresh/spec.md) | Copied-result refresh and selected-source observations |
| [output-policies](output-policies/spec.md) | Retention demand, protection and reclaim eligibility |
| [policy-resources](policy-resources/spec.md) | Immutable policy definitions, lookup and explicit adoption |
| [application-claims](application-claims/spec.md) | Request-bearing associations, context metadata and primitive claim operations |
| [claim-query](claim-query/spec.md) | Application-scoped context matching, public claim discovery and consistent snapshots |
| [claim-recovery](claim-recovery/spec.md) | Explicit unavailable-content recovery and exact restoration |
| [claim-security](claim-security/spec.md) | Record integrity, optional credentials and application access policies |
| [consumer-errors](consumer-errors/spec.md) | Stable native and CLI failure codes and structured diagnostics |
| [application-deactivation](application-deactivation/spec.md) | Binding-only deactivation and later reactivation |
| [application-lifecycle](application-lifecycle/spec.md) | Coordinated install/update/rollback and logical uninstall |
| [lifecycle-recovery](lifecycle-recovery/spec.md) | Operation identity, phases, retries, recovery and integrated acceptance |
| [shim-maintenance](shim-maintenance/spec.md) | Launcher ownership, adoption, replacement and verified cleanup |
| [self-management](self-management/spec.md) | Saucepan's use of the shared managed application contracts |
| [resource-access](resource-access/spec.md) | Exact-output lookup and native/process lifetime ownership |
| [group-leases](group-leases/spec.md) | One invocation lifetime for named prepared outputs |
| [resource-groups](resource-groups/spec.md) | Registered intent, atomic preparation and selected generations |
| [output-pins](output-pins/spec.md) | Explicit durable retention of exact outputs |
| [language-sdks](language-sdks/spec.md) | Typed Python/TypeScript operations, explicit identity, transport failures and native lifetimes |
| [sdk-distribution](sdk-distribution/spec.md) | Source/package installation, release evidence and selective SDK checks |
| [content-verification](content-verification/spec.md) | Metadata/full checking and mandatory integrity boundaries |
| [lease-marker-cleanup](lease-marker-cleanup/spec.md) | Process-marker creation and safe stale-marker cleanup |

## Shared meanings

Root selection is owned by store-layout: direct CLI use defaults to the home
`.saucepan`, explicit overrides win, and native callers and managed Saucepan
shims retain explicit roots. Examples using an explicit root do not make it a
required CLI flag.

Source queries search registered definitions, without claim scope. Claim queries
search owned context and optionally an exact scope; anonymous claim discovery
requires public scope exposure. Public source visibility is a separate setting.
Credentials are opt-in and never replace source, scope or action permissions.
App sessions enforce those permissions; trusted store-wide administration is a
separate boundary, not a fallback after a denied application call.

An application claim associates a request with an output; it is not a deletion
pin. An active binding selects what a managed launcher runs. A resource lease
protects a specific output independently of later claim or binding changes.
Retention, active protection, leases and validated operation intent all contribute
to reclaim eligibility; removing one does not cancel the others.

Application `ready` means the registration accepts ordinary operations; it does
not require an active executable. Output `ready` means published content is
available under the output contract. A `complete` operation is a recorded result,
not a guarantee that no later operation changed the application.

Primitive `update-claim` moves a claim, `activate` changes a binding, and
`deactivate` clears a binding. Managed `update-app`, `self-update` and `rollback`
coordinate a selected claim with a binding. Uninstall disables the registration
before owned-file cleanup; output reclaim remains a separate operation.

An operation's terminal `conflict` requires a new operation ID for another
attempt. A file-identity conflict recorded as a pending reason leaves the
existing maintenance/removal operation resumable after the cause is resolved.
Inspection reports state; reconciliation resumes recorded committed work;
unfinished preparation requires retrying the original command.

The original mock installation is retained as inspectable acceptance evidence.
Subsequent lifecycle acceptance uses an isolated simulated root and verifies
that the existing mock remains unchanged. Runtime evidence is host-specific;
passing validation of these documents does not establish cross-platform execution.
