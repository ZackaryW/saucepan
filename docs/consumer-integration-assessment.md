# Consumer integration assessment — 2026-10-14

This assesses features, not compatibility with the former Saucepan protocol.
The earlier source inspection covered Power Eagle at
`2f3accf42903b624a747820688fccb5bc3c4bcdb` and Zush at
`96c91a79811fc1a8a838c2f50c88d26e62691d56`. Neither external application was changed
or executed for this implementation. Zush already had a modified `vendor/saucepan`
submodule at that inspection.

The implemented `orchestrate-resource-groups` change adds persistent registration,
policy-driven preparation, atomic selected generations and explicit durable output
pins. Its transaction covers Saucepan metadata; downloads, caller-owned directories,
plugin activation and external application state remain outside that transaction.

## Feature fit

| Consumer | Saucepan now supplies | Consumer still owns |
| --- | --- | --- |
| Power Eagle | Git/release acquisition, copied plugin trees, coherent group selections, and explicit pins that survive the updater process | Catalog interpretation, package/manifest validation, imports, activation, switching consumers and releasing obsolete pins |
| Zush | Git refs/commits and subpaths, independent scoped claims, saved concrete resource groups, preparation and retained command/hooks/service launch | Wildcard/catalog expansion, command parsing, Python environments, hook ordering and service cleanup |

Both can use local basic-integrity stores without credentials, keyring access or a
Python binding. Opting into an app applies its source/scope/action restrictions even
with credentials disabled. Temporary/no-store composition remains available outside
persistent groups; persistent groups coordinate managed cached outputs.

## Power Eagle

The observed host imports installed ESM plugins in its existing process:
`src/host/install/disk-plugin.ts` reads the manifest and dynamic-imports its entry,
while `src/host/install/saucepan.ts` uses a short `spawnSync` call. Returning a bare
path from that call did not retain the files after the updater exited.

Use this flow:

1. Resolve a catalog entry to registered source, selector, selection and policy IDs.
2. Acquire and explicitly pin A, or register a group and prepare-and-pin A atomically.
3. Validate the plugin manifest and entry containment, then import/read the returned
   immutable store paths directly.
4. Prepare and pin B under a new consumer key. Switch the host's selected plugin.
5. Unpin A only after all old modules and delayed asset readers have finished.

A pin keeps A's exact bytes addressable through Saucepan reclaim, including under
`history: none`. It survives process exit and does not follow a moving claim.
Independent pins keep independent retention. A lost prepare-and-pin response can
be reconciled with the same key and expected group revision without another fetch.
A crashed host may leave a pin; inspection and explicit release handle cleanup.

There is no need for destination sync pairs, runtime copy claims, lifecycle copy
hooks, an IPC lease holder or a second plugin copy merely to preserve lifetime.
Generated/editable plugins and runtime state still belong outside immutable store
payloads. Building packages and installing JavaScript dependencies remain outside
Saucepan. The consumer must validate `manifest.main` containment and coordinate
activation failures; storage transactions cannot undo executed JavaScript.

## Zush

The observed `src/zush/core/acquisition.py` resolves commands and gathers extensions;
`runtime.py` runs Python callables and keeps generator services through cleanup.
`core/workspace.py` previously returned bare acquired paths. This needs a retained
lifetime spanning discovery, imports, hooks, commands and service shutdown.

Resolve wildcard subscriptions and aliases into concrete members, register an
owner-local group key, then launch a dedicated execution child with
`prepare-run-group GROUP_UUID -- PROGRAM`. Consume `SAUCEPAN_RESOURCE_GROUP_FILE`
without independently reacquiring newer selections inside that invocation.
Use `run-registered-group` when preparation is intentionally separate. Native
consumers can retain the returned `GroupLease` directly.

Compatible Git subpaths are prepared from one exact resolution; manual members can
retain their previous version. Failure preserves all prior claim selections and
successful-check timestamps. Zush should publish its own recipe/catalog state only
after the group receipt. Catalog enumeration, brand rules and subscription expansion
remain Zush features: include every resource they need in the concrete group.

Detached services and descendants that close inherited handles need their own
supported lifetime arrangement. The earlier audit did not establish propagation
through Zush's complete Python/subprocess chain. An explicit pin is an alternative
when the consumer can reliably decide when retention should end.

## Evidence and remaining work

[Output pins](output-pins.md) and [registered groups](resource-groups.md) document
current payloads. The isolated Rust/CLI fixtures cover updater exit, delayed reads,
independent pins, A/B revisions, claim movement, shared Git commits, partial failure,
reclaim during use, and metadata-journal recovery. The [verification report](resource-group-verification.md)
records exact local checks separately from actual application integration.

Neither app is a drop-in integration yet. Validate real packaged ESM imports,
activation failure and host shutdown in Power Eagle. Validate Zush declaration
checks, KeyboardInterrupt, Python/service cleanup and surviving script children.
No compatibility decoder, external adapter, package-manager runtime, hosted CI run,
or consumer repository modification is part of this change.
