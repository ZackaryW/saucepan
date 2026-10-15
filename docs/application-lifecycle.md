# Application lifecycle

[← Saucepan guide](../README.md#managed-applications-and-releases)

CLI commands use the user-home `.saucepan` store by default. Examples below use
`--root ROOT` to keep a selected installation isolated. Commands identifying an
application apply its registered policies; supply `--credential-file FILE` when
that app requires proof. Global reconciliation is trusted local maintenance.

## First setup with a profile

Obtain and extract the [release ZIP for your platform](releases.md). Keep the
manager and matching `saucepan-shim` together. SDK installation does not download
these binaries. The following PowerShell example assumes a Windows x64 release
extracted into the current directory; change the asset pattern and executable
names for another platform. `ROOT` below is an explicit isolated destination;
omit `--root ROOT` to use your home `.saucepan` store.

```powershell
$manager = (Resolve-Path ./saucepan.exe).Path
$shim = (Resolve-Path ./saucepan-shim.exe).Path
& $manager --root ROOT init
$profile = & $manager profile explicit-update --for-install | ConvertFrom-Json
$filterId = [guid]::NewGuid().ToString()
$filter = @{uuid=$filterId; type='github_release_asset_filter'; asset_patterns=@('saucepan-*-x86_64-pc-windows-msvc.zip')}
@($profile.catalog) + @($filter) | ConvertTo-Json -Depth 30 | Set-Content -Encoding utf8 setup-policies.json
& $manager --root ROOT register-policies setup-policies.json
@{type='github_release'; repo='ZackaryW/saucepan'} | ConvertTo-Json | Set-Content -Encoding utf8 source.json
$source = & $manager --root ROOT register-source source.json | ConvertFrom-Json
$request = @{
  source_uuid=$source.uuid; selector=@{type='latest_stable'}
  selection=@{format='zip'; bin=@('saucepan.exe'); filter_refs=@($filterId)}
  policy_refs=@($profile.combined_policy_refs)
}
$request | ConvertTo-Json -Depth 30 | Set-Content -Encoding utf8 bootstrap.json
$operation = [guid]::NewGuid().ToString()
& $manager --root ROOT bootstrap bootstrap.json $shim --operation $operation
```

Check the returned operation's `status`; only `complete` means setup finished.
Retain its app/claim identifiers and original request/operation ID for inspection
and retry. The profile adds ordinary usable-app protection, while bootstrap
supplies Saucepan's fixed self scope and version probe. Policy/source registration
and cached material may survive a failed installation; they are not proof of a
live app. The temporary profile cannot install a stable executable.

Add the resulting root's `bin` directory to PATH using your shell/system settings,
then open a new shell and run `saucepan --version`. For an isolated root you can
instead invoke `ROOT/bin/saucepan.exe` directly. Saucepan does not edit PATH or
shell configuration. Use the returned identifiers with `self-update APP_UUID
CLAIM_UUID`; downloading a release alone does not activate it. Generic managed
apps use the same expanded refs with the explicit `install` command below, their
own source, scope and probe. Resource-only consumers can use a profile without
registering an application or installing an executable.

## Lifecycle and recovery

Application metadata includes readiness/removing state, a monotonic mutation
revision and an owned-shim list. Only the current store integrity contract is
supported; earlier layouts are rejected without migration. Removing applications reject claims,
settings, activation and deactivation mutations; `inspect` can still show the
record. A no-op deactivation or unchanged settings does not advance revision.

Durable lifecycle operations live under
`.staging/lifecycle/<operation-uuid>/index.json`. These are recovery receipts,
not disposable download staging. The short journal couples metadata changes
with their operation receipt. App retirement removes integrity-verified app/registry
membership in the same transition; verified recovery receipts remain available.
Unknown additional files are never recursively deleted.

```powershell
saucepan --root ROOT operations
saucepan --root ROOT operations --app APP_UUID
saucepan --root ROOT reconcile --operation OPERATION_UUID
```

Inspection does not resume work. `operations --app APP_UUID` is app-scoped and
requires a live registration and its applicable credentials/policies. After
uninstall, use administrative `operations` without `--app` to inspect retained
receipts. Reconciliation reports a JSON list of operation
results; all complete results exit 0, while pending/failure/conflict results exit
2. Unknown operation identifiers are errors. Interrupted preparation requires
retrying its original command so candidate validation is performed again.
Opening the store replays committed metadata only; it does not probe programs,
download resources or wait for a busy launcher.

An operation identifier with unchanged inputs resumes the same operation or
returns the recorded terminal result. Different inputs under that identifier
are rejected. Same-operation execution is serialized. Terminal failure needs a
new identifier for a new attempt. Keep receipts until an explicit future
compaction policy exists; time and PIDs are not proof of abandonment.

Lock order is operation, shim maintenance, then short store mutation; candidate
scans and probes run outside store mutation while retaining output protection.
File cleanup belongs to explicit reconciliation, not global store recovery.
Only binaries implementing the selected integrity contract may use its root. There
is no older-schema reader or writer compatibility promise. Use untouched separate
roots when switching back to a previous implementation.

Owned launcher maintenance uses these commands:

```powershell
saucepan --root ROOT install-shim APP_UUID tool.exe BUILT_SHIM --operation OPERATION_UUID
saucepan --root ROOT replace-shim APP_UUID tool.exe BUILT_SHIM
saucepan --root ROOT adopt-shim APP_UUID tool.exe MATCHING_BUILT_SHIM
```

Use `--saucepan` consistently for Saucepan's root-forwarding launcher. Replacement
cannot change the owner or root-forwarding mode. Both the executable and its
derived `.shim.json` name are reserved, including case aliases. Adoption verifies
the existing build and configuration before registering ownership; it replaces
neither file. Replacement can restore missing owned files but will not overwrite
unexpected contents. Unknown files and filesystem links are conflicts.

Multi-step commands return the operation record, including status and pending
reasons. A running Windows launcher holds a maintenance lock: replacement can
report `commit_intent` with exit 2 until it exits. Use the returned operation ID
with `reconcile` afterward. The original program remains usable; target output
leases are separate from launcher maintenance. Existing installed launchers gain
this extra coordination only after explicit replacement with the new build.

Managed application commands share candidate validation and operation reporting:

```powershell
saucepan --root ROOT install example request.json probe.json BUILT_SHIM --shim example.exe --scope example
saucepan --root ROOT update-app APP_UUID CLAIM_UUID probe.json
saucepan --root ROOT rollback APP_UUID CLAIM_UUID RETAINED_ENTRY_UUID probe.json
```

Install registers the app only after a usable candidate and complete shim pair
are ready. Update and rollback commit the selected claim and active binding
together; failed probes preserve both. Rollback requires the same item and
keeps the claim's original request, including its tracked selector. It performs
no remote discovery. Candidates remain leased through preparation, and durable
commit intent protects them from reclaim after the preparing process exits.
Reconciliation verifies their integrity before finishing a committed intent.

`bootstrap` and `self-update` use these same operations with the Saucepan release
request and version probe. Their JSON results now use the lifecycle operation
shape: `app`, `claim`, `candidate`, `status`, `operation` and `desired` (the
committed application snapshot on success), rather than `claimed`/`binding`.
The shim commands likewise return operation records. `--operation UUID` is
optional on every multi-step command; retain it to retry a lost response.

An intervening app mutation makes a managed update or rollback conflict instead
of replacing newer intent. Retry with a new operation ID. The lower-level
`activate` still changes only a binding and may commit after deactivation;
`update-claim` still changes only a resource claim. Neither silently acquires
the other operation's meaning. Uninstall fences both kinds of later commit.

```powershell
saucepan --root ROOT uninstall APP_UUID
saucepan --root ROOT reconcile --operation REMOVAL_OPERATION_UUID
```

Uninstall first marks the application removing, clears its claims/binding and
rejects later mutation or launch. It then deletes only verified owned launchers
and configurations. Existing native guards and target processes retain their
output protection. Changed files remain untouched with a pending reason; missing
owned files count as already cleaned. Unknown app-directory files also remain
untouched. Other apps remain usable while cleanup is pending.

Unregistered matching shims require explicit adoption or
`uninstall APP_UUID --leave-unmanaged-shims`; the latter reports their names as
leftovers. A completed removal is repeatable and returns its recorded result.
An arbitrary missing application UUID is an error.

On Windows, self-uninstall through Saucepan's own shim normally reports pending
launcher cleanup until the wrapper exits. Finish reconciliation using a directly
invoked retained or newly built Saucepan binary with `--root ROOT`. The command
does not terminate targets or schedule a hidden cleanup service.

Uninstall never deletes source working data or cached outputs. Later `reclaim`
still uses surviving output policies, other claims/bindings and live leases.
Removing a large-retention claim does not necessarily reduce the effective
window while cached output records still carry that policy demand.
