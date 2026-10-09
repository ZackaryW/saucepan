# Specification map

These main specs describe the current Saucepan contract. Archived changes record
historical decisions and are not alternative versions to inherit at runtime.
Adding a provider or policy type does not establish backward schema compatibility.

Read overlapping requirements together using these responsibility boundaries:

| Capability | Owns |
| --- | --- |
| [store-layout](store-layout/spec.md) | Root layout, current data contract and rejection of pre-reimplementation stores |
| [source-resolution](source-resolution/spec.md) | Provider resolution, discovery freshness and asset selection |
| [output-acquisition](output-acquisition/spec.md) | Immutable output identity, acquisition and reuse |
| [output-policies](output-policies/spec.md) | Retention demand, protection and reclaim eligibility |
| [application-claims](application-claims/spec.md) | Request-bearing associations and primitive claim operations |
| [application-deactivation](application-deactivation/spec.md) | Binding-only deactivation and later reactivation |
| [application-lifecycle](application-lifecycle/spec.md) | Coordinated install/update/rollback and logical uninstall |
| [lifecycle-recovery](lifecycle-recovery/spec.md) | Operation identity, phases, retries, recovery and integrated acceptance |
| [shim-maintenance](shim-maintenance/spec.md) | Launcher ownership, adoption, replacement and verified cleanup |
| [self-management](self-management/spec.md) | Saucepan's use of the shared managed application contracts |
| [resource-access](resource-access/spec.md) | Exact-output lookup and native/process lifetime ownership |
| [content-verification](content-verification/spec.md) | Metadata/full checking and mandatory integrity boundaries |
| [lease-marker-cleanup](lease-marker-cleanup/spec.md) | Process-marker creation and safe stale-marker cleanup |

## Shared meanings

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
