# Consumer integration assessment — 2026-10-14

This is a feature assessment of the current local implementations, not a proposal
to retain their old Saucepan protocols. Power Eagle was inspected at
`2f3accf42903b624a747820688fccb5bc3c4bcdb`; Zush at
`96c91a79811fc1a8a838c2f50c88d26e62691d56`. Power Eagle was clean; Zush already
had a modified `vendor/saucepan` submodule. Neither repository was changed.
Saucepan's assessed implementation is `83d8a53`, with its finalized group specs
archived in `7708a38`.

The separate active plan is `orchestrate-resource-groups`: persistent group
registration, explicit preparation and atomic selected generations. It is not
implemented. Its managed-output boundary does not extend to atomic publication
of arbitrary caller-owned directories, temporary groups or external app state.

## Overall assessment

| Consumer | Existing feature fit | Main remaining integration issue | Benefit from the new plan |
| --- | --- | --- | --- |
| Power Eagle | Strong for supplying complete plugin directories and independently versioned assets | Plugins execute inside an already-running Eagle host; a short CLI lookup does not retain files for its lifetime | Saves plugin-set intent and prepares coherent managed generations, but does not supply an in-process lease bridge or atomic plugin activation |
| Zush | Strong for Git selections, copied trees, source provenance and one invocation containing command/hooks/services | Current adapter returns paths only; all resource reads and execution need a retained lifetime | Directly removes repeated group assembly and partial multi-claim selection updates |

Neither is a drop-in integration today. That is more than a command-renaming
problem: ownership lifetime and consumer-specific discovery need deliberate
wiring. Neither requires mandatory app credentials, a keyring or a new Python
binding to use basic local acquisition.

## Shared capabilities already present

- Local files/directories, URLs, Git and GitHub release assets are supported.
  GitHub-repository input normalizes to Git. Git ref/tag/commit selection and
  copied subpaths fit both consumers; release filtering can select platform assets.
- Retrieval returns original material; acquisition produces copied/exported or
  extracted files. Managed, temporary and caller-owned output destinations have
  distinct ownership. Consumers must not treat a bare Git source path as a plugin
  or Python package directory.
- Cached retrieval freshness, acquisition updates, identical reuse, revision
  history and reclaim recovery are separate policy axes. Defaults do not require
  callers to construct policy objects. No-store/temporary retrieval deliberately
  cannot produce persistent managed claims or history.
- App registration, claim scopes, credentials and payload/path verification are
  independent choices. Basic store metadata checks remain mandatory; optional
  HMAC custody is not needed for ordinary local use. Selecting an app still applies
  its restrictions even when credentials are disabled.
- Claim queries can filter explicit scope/context. Generic source queries inspect
  registered, visible metadata; they do not enumerate GitHub accounts or parse
  application package manifests. Anonymous discovery includes explicitly public
  sources, not every locally stored source.
- Current group leases protect up to 128 named existing output/claim views under
  one lifetime and optional restrictive policy boundary. They do not fetch or
  update members, create claims or save a group registration.

Evidence: [source models](../src/models/source/mod.rs),
[selectors](../src/models/store/mod.rs), [destinations](../src/models/store/composition.rs),
[composition guide](composition.md), [source queries](../src/core/query/source.rs),
[claim queries](../src/core/query/mod.rs), [group leases](group-leases.md).

## Power Eagle

The observed consumer is a plugin host, not merely a template downloader.
`src/app/host-service.ts:183` resolves installed directories;
`src/host/install/disk-plugin.ts:41` reads `manifest.json` and dynamic-imports its
`main` ESM entry into the host. Service/styling plugins also participate through
`resolveHostContext` at line 144. The README says installed service/styling
activation is absent, but this code already calls their loading/activation path;
this assessment follows the implementation.

| Need | Assessment and recommendation |
| --- | --- |
| Fetch complete prebuilt ESM packages | Existing Git/subpath and release/archive acquisition supply the files. Keep package manifest interpretation, plugin ID/name/kind and export validation in Power Eagle. Saucepan does not need to become a plugin registry. |
| Read plugin files for the full host lifetime | **Current integration gap.** `src/host/install/saucepan.ts:60` uses `spawnSync`; `installedPath` returns a string after the process exits. A future `run-group` child cannot transfer a native guard back into the already-running parent webview. Prefer acquisition into a create-new, caller-owned version directory for this host. Power Eagle controls when old copies can be deleted. If shared cache residency is essential, design an explicit long-lived holder/IPC session or host launch arrangement and test its shutdown behavior; that is not part of the new plan. |
| Plugin catalogs and buckets | **Consumer catalog work.** The present UI asks Saucepan for bucket URLs and maps `sauce` fields into plugin summaries (`host-service.ts:37`). Current Saucepan stores resource metadata, not that catalog format. Power Eagle should resolve its catalog's names to source/selector/selection and retain its own manifest projection. A reusable bucket/catalog feature would be a separate product decision. |
| Several enabled plugins update together | **Planned for managed selections.** The new group plan can prepare files and publish their selected generation atomically. Power Eagle must still coordinate imports, activation, disabling old modules and its own selected-directory pointer. Its existing `buildHostContext` intentionally records individual activation failures; storage atomicity cannot roll back already executed JavaScript. Caller-owned copy sets need their own app publication transaction. |
| Editable/generated plugins | **Consumer-owned state.** `src/ai/conversation-store.ts:75` writes versioned generated plugins. Keep that workspace outside immutable managed store payloads. Finished generated versions can later be used as local sources. |

Recommended flow: resolve a plugin catalog entry → retrieve the chosen source →
acquire into a new Power Eagle-owned version directory → validate its manifest
and entry containment → publish Power Eagle's selection → import and activate it.
Keep prior copies while their modules or assets may still be used. Source caching
can remain shared even when the final copy belongs to Power Eagle.

Separate from compatibility, `disk-plugin.ts` currently joins `manifest.main`
without an explicit containment check. The consumer should validate that entry
before import; safe archive extraction does not validate an application's manifest
semantics. Likewise, a prebuilt plugin is expected: package builds and JavaScript
dependency installation are not Saucepan features.

## Zush

`src/zush/core/acquisition.py:265` resolves a command, acquires its repository,
revalidates the declaration, gathers extensions and calls `runtime.execute`.
That runtime invokes Python callables in-process and retains generator-backed
services until cleanup (`runtime.py:74`); script commands/hooks spawn children
(`runtime.py:36`). This is a natural finite group lifetime.

| Need | Assessment and recommendation |
| --- | --- |
| Branches, exact commits, repository packages and subpaths | Existing Git acquisition fits. Preserve enough of the tree for relative imports and package dependencies; a command file alone may not be a complete acquisition. `utils/resolve.py:41` already handles invocation-scoped imports and package collision checks. |
| Command, hooks, services and brand extensions available together | **Existing group feature; integration missing.** `core/workspace.py:62` and `:75` return acquired/bare paths, while `Application.roots` keeps those strings. Gather the exact resource set, retain it before discovery/import and hold it through hook execution and service cleanup. Include custom brand rule/parser/dispatcher resources used by `core/export.py:12`, not only the command's repository. |
| Retention through subprocesses and failures | Run a dedicated Zush execution child under `saucepan run-group`; consume the descriptor and avoid reacquiring newer selections inside that child. The invocation can keep its inherited guards while it waits for scripts. `runtime.py:60` does not explicitly propagate Saucepan lifetime handles to script descendants, so survival after the Zush parent dies is not established. Test that case or launch independently surviving work through its own supported lifetime boundary. |
| Persisted sets and automatic preparation | **Planned.** Registered groups plus prepare-and-run can replace repeated resource assembly. App claims remain optional: local output groups need no app; application groups preserve explicit independent scopes and current restrictions. |
| Atomic bulk updates | **Planned for Saucepan selections.** `Application.manage` at line 113 currently collects per-repository successes/failures and saves state during each acquisition. The proposed group preparation gives all-or-nothing claim/group publication. Zush must update its own `selected`, recipes and catalog projection only after that receipt, and decide whether its user-facing bulk command retains partial-success mode or explicitly opts into an atomic group. |
| Wildcards, remote enumeration and aliases | Keep these in Zush. `utils/gh.py:32` paginates authenticated GitHub listings; `core/subscriptions.py:31` stores standing wildcard intent. Saucepan query filters do not discover unregistered upstream repositories. Expand a wildcard to concrete resources before group registration and revision-check membership updates. `core/catalog.py:28` retains command aliases and catalog validation. |
| Refresh on every invocation | Choose acquisition update/reclaim policies explicitly. Source freshness alone does not move an existing manual claim. Preserve Zush's existing rule that unknown command leaves do not trigger downloads, and revalidate declarations after any prepared generation change. |

Recommended flow today: discover enough metadata to identify the invocation's
concrete resources → ensure/acquire them individually → form a named group →
launch a dedicated Zush execution child with the group descriptor → perform final
discovery/declaration checks and run hooks/command/services under that lifetime.
For discovery that reads cached files, retain those reads too; if discovery changes
the required set, build and validate the final group before running user code.
After implementation of the new plan, registration plus prepare-and-run can own
the preparation/selection phase. Python package environment management, command
parsing, hook ordering and generator/service cleanup remain Zush responsibilities.

## Remaining boundaries after the proposed change

1. **In-process foreign-runtime retention:** no attachable IPC lease session for
   an existing JavaScript/Python parent. Caller-owned copies or a supported child
   launch are available alternatives; no PyO3 package is required.
2. **Application discovery:** catalogs, command aliases, bucket interpretation and
   upstream wildcard subscriptions are not supplied by group registration.
3. **External atomicity:** plugin activation, hooks, user config and caller-owned
   directory switching are outside Saucepan's metadata transaction.
4. **Fresh multi-resource workflows:** standalone temporary/no-store composition
   exists, but the proposed persistent groups coordinate managed output only.
5. **Real runtime evidence:** neither Eagle's webview lifecycle nor Zush's complete
   Python/service/subprocess chain has been validated against this implementation.

## Evidence and next acceptance checks

This turn inspected source, specifications and existing test evidence; it did
not execute either application, download packages or run hosted CI. Existing
Saucepan [group verification](group-lease-verification.md) records 283 passing
tests on Windows with five optional native-keyring tests ignored. Those fixtures
prove Saucepan contracts, not end-to-end interoperability with these apps.

Before calling either integration seamless, exercise:

- Power Eagle: real packaged plugin import, delayed asset/module reads while
  Saucepan reclaims old cache, replacement with an old instance still running,
  invalid manifest entry, activation failure and host shutdown cleanup.
- Zush: repositories supplying command/hooks/services and brand extensions,
  repeated invocation, upstream changes, a failing member, reclaim during use,
  KeyboardInterrupt and parent death with a still-running script child.
- Both: local no-app/basic mode and opted-in scoped access, exact policy adoption,
  explicit unavailable-content handling and an offline cached path.

The highest-value order is the Zush retained execution integration and Power
Eagle caller-owned delivery, followed by adoption of prepared persistent groups
where coherent multi-resource selections are actually needed.
