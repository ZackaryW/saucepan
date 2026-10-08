# Application lifecycle

Application metadata includes readiness/removing state, a monotonic mutation
revision and an owned-shim list. Existing new-format records load as ready with
revision zero and no registered shims. Removing applications reject claims,
settings, activation and deactivation mutations; `inspect` can still show the
record. A no-op deactivation or unchanged settings does not advance revision.

Durable lifecycle operations live under
`.staging/lifecycle/<operation-uuid>/index.json`. These are recovery receipts,
not disposable download staging. The short journal couples metadata changes
with their operation receipt. App retirement moves its validated directory to
the operation directory so interruption cannot leave a broken index inventory.
Unknown additional files are never recursively deleted.

```powershell
saucepan --root ROOT operations
saucepan --root ROOT operations --app APP_UUID
saucepan --root ROOT reconcile --operation OPERATION_UUID
```

Inspection does not resume work. Reconciliation reports a JSON list of operation
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
Current and older writers must not mutate the same root concurrently. Reading
old records does not promise that old binaries can write the new metadata.

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
