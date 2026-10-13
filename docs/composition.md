# Retrieval and acquisition contracts

## Flat policy integration

The reference-based native entry points are `Retriever::retrieve_referenced`,
`Store::retrieve_referenced`, `Store::compose_referenced`, and their scoped
`AppSession` equivalents. `Acquirer::acquire_referenced` copies a retained source
handle offline; `Acquirer::compose_referenced` performs root-free fresh retrieval
and copying. Metadata-only `RegisteredRetriever` has the same reference-based
operations. CLI composition commands use these same reference-only inputs.

Create a caller-owned `PolicyCatalog` from standalone definitions:

```json
[
  {"uuid":"10000000-0000-4000-8000-000000000001","type":"cache_policy","mode":"temporary"},
  {"uuid":"10000000-0000-4000-8000-000000000002","type":"update_policy","mode":"always"},
  {"uuid":"10000000-0000-4000-8000-000000000003","type":"revision_policy","history":"latest","keep":3},
  {"uuid":"10000000-0000-4000-8000-000000000004","type":"cache_policy","mode":"no_store"}
]
```

Deserialize a `models::policy::requests::Retrieval`:

```json
{
  "source":{"kind":"inline","source":{"type":"local","path":"fixtures/input"}},
  "selector":{"type":"local"},
  "policy_refs":["10000000-0000-4000-8000-000000000001"]
}
```

Pass it and the catalog to `Retriever::retrieve_referenced`. To copy its contents,
deserialize `models::policy::requests::Acquisition` and call
`Acquirer::acquire_referenced` while retaining the retrieved handle:

```json
{"destination":{"mode":"temporary"},"policy_refs":[]}
```

These operations require no home directory, store, application, or keyring. The
two returned handles own independent lifetimes. `no_store` instead requires a
separate retrieval `destination` path and never overwrites an occupied destination.

Managed operations use definitions registered with `Store::register_policies`;
pass an empty portable catalog when all references are registered. An unregistered
definition fails before provider work or publication. Selecting the update policy
on acquisition controls copied-result checks; selecting the revision policy controls
managed history. Neither belongs on the retrieval request. Release asset selection
uses ordered `asset.filter_refs`, preserving platform filtering and ambiguity errors.

Execution records retain definition UUID/digest evidence. Equivalent UUIDs do not
change content identity. Relevant source/application attachment changes fence
publication; unrelated catalog registration does not invalidate the operation.
The executable fixtures are in `tests/policy_composition.rs`.

## Independent operations and lifetimes

The new request models separate three policy decisions. Native inline fresh
retrieval is available through `Retriever`, and managed retrieval through `Store`.
Offline acquisition, scoped automatic updates and CLI consumers use the same native contracts.

| Policy | Responsibility | Managed default |
| --- | --- | --- |
| Retrieval cache/freshness | Obtain original source material and decide whether to cache it | Cached, check after 86,400 seconds |
| Acquisition update | Decide when to check whether copied results have changed | Manual |
| Revision history | Decide how many acquired revisions to retain | Latest ten |

`models::policy::requests::Retrieval` selects a source, revision and original asset.
Git retrieval returns repository data and an exact commit; a ZIP stays an archive.
Folder selection and extraction belong to `models::policy::requests::Acquisition`.

For example, retrieve a fresh archive into an explicitly caller-owned destination:

```json
{
  "source": {
    "kind": "inline",
    "source": { "type": "url", "url": "https://example.invalid/tool.zip" }
  },
  "selector": { "type": "url" },
  "destination": "download/tool.zip",
  "policy_refs": ["10000000-0000-4000-8000-000000000004"]
}
```

Then acquire its files into another caller-owned location:

```json
{
  "destination": { "mode": "directory", "path": "unpacked/tool" },
  "format": "zip",
  "policy_refs": []
}
```

Both destinations must be new and must not overlap. Validation of the JSON is
pure; runtime operations must also validate paths, access and ownership before
fetching or writing.

| Retrieval | Managed output | Temporary output | Caller-owned output |
| --- | --- | --- | --- |
| Cached | Supported | Supported | Supported |
| Temporary | Rejected | Supported | Supported |
| No store | Rejected | Supported | Supported |

Temporary and no-store retrieval always fetch fresh without Saucepan cache reuse.
Temporary results require a live native guard or a supported consumer process;
a serialized path cannot retain that lifetime. Caller-owned destinations survive
the operation and are never reclaimed by Saucepan. Descriptors report ownership
and evidence but cannot manufacture a managed lease or authorize access.

Native handles are `core::retrieval::RetrievedResource` (`source_path()`) and
`core::store::AcquiredResource` (`path()`). Their fields are private and they do
not implement deserialization or cloning. Their `descriptor()` references expose
observational metadata separately from native lifetime ownership.

For fresh native retrieval, pass a `Retrieval` and its catalog to
`Retriever::retrieve_referenced(&request, &catalog, &options)`. This entry point accepts inline
temporary/no-store requests without opening a store or locating a home directory.
It rejects registered/cached requests that require an owning store. Keep temporary
handles alive while reading their paths. No-store publication transfers ownership
to the caller. Git delivery is a directory containing `repo` and dependencies;
`source_path()` identifies the bare `repo` directory inside it. Downloads and local
files use the exact requested destination filename. Parent directories must exist.

The native example `cargo run --example retrieve-source -- REQUEST.json` inspects
source metadata while retaining the handle. It emits a descriptor only for
caller-owned results; temporary paths are not advertised after their owner exits.

Managed native retrieval uses `store.retrieve_referenced(&request, &catalog, &options)` with cached mode.
It registers an inline source or uses a registered UUID, publishes a retrieval
record under `index/<uuid>/index.json`, and stores original non-Git material under
`source/<source-uuid>/retrieval/<uuid>/material`. Git repository/dependency caches
remain under the source directory. No acquired output or claim is created.
`store.source_path(uuid)` is a bare address; `store.lease_retrieval(uuid)` retains
coordinated access without refreshing. `store.purge_retrieval(uuid)` returns false
while source consumers are live; after they release, retry to retire that record.
Purge does not prune shared Git objects or delete acquired copies or registrations.

`store.configure_source_freshness(uuid, policy)` sets the native retrieval default.
Omitting a request refresh policy inherits registered defaults, then the built-in
24-hour freshness. Per-call overrides do not modify the source. Original material has
its own journaled publication and integrity checks, separate from output reclaim.

Only managed outputs accept references to `revision_policy` definitions. `{"history":"none"}` means keep
the newest revision; `{"history":"latest","keep":3}` includes the newest among
the three. Live leases, active protection and other surviving retention demands
can defer deletion. `RevisionPolicy` is the single history representation; no
eviction adapter changes `none` into `latest, keep: 1`.

Update modes are `manual`, `always`, and `max_age` with `seconds`. A due update
checks the selected content before reuse, even within the retrieval cache TTL.
It never changes an explicit pinned commit. A changed repository head with an
unchanged selected folder may reuse old bytes; checked and selected provenance
remain distinct. Due updates conflict with `cache_only` retrieval.

Payload verification is `none`, `if_available`, or `required`. An explicit digest
cannot be combined with `none`. Required verification needs an expected digest;
a self-computed storage identity is not proof of upstream authenticity. Structural
validation and managed metadata/manifest integrity retain their own contracts.

For offline acquisition, use `Acquirer::acquire_referenced(&retrieved, &selection, &catalog, scope)` with
manual update mode. Managed destinations use the retrieval's owning store;
temporary and directory destinations create independent copies. The source guard
may be dropped after copying. Temporary acquired paths last until their handle
is dropped; directory destinations remain caller-owned.

One repository can supply two subpaths without another provider request:

```rust
use saucepan::core::store::Acquirer;
use saucepan::models::{policy::requests::Acquisition, store::Destination};

let repository = store.retrieve_referenced(&git_request, &catalog, &options)?;
let source_path = repository.source_path(); // bare repository, exact commit in descriptor
let first = Acquirer::acquire_referenced(&repository, &Acquisition {
    folder: Some("plugins/first".into()),
    destination: Destination::Temporary {},
    ..Default::default()
}, &catalog, None)?;
let second = Acquirer::acquire_referenced(&repository, &Acquisition {
    folder: Some("plugins/second".into()),
    destination: Destination::Temporary {},
    ..Default::default()
}, &catalog, None)?;
// first.path() and second.path() are copied directories, independent of source_path.
```

The integration fixture in `tests/acquisition_composition.rs` runs this pattern
with the Git origin removed after retrieval. It also verifies independent output
lifetimes and cached ZIP extraction surviving source-material purge.

Managed automatic updates use `store.compose_referenced(&retrieval, &acquisition, &catalog, &options)`.
This entry point owns the fetch/lease order; use it when `update_policy` is
`always` or `max_age`. A borrowed handle remains an offline input. To release a
handle and perform an update, use `Acquirer::acquire_or_refresh_referenced(handle, &acquisition, &catalog, scope, &options)`;
previous acquired-output handles can remain alive throughout the update.
`Acquirer::compose_referenced` composes inline fresh operations without a store.

For example, keep retrieval's 24-hour default, set acquisition update to
`UpdatePolicy::MaxAge { seconds: 60 }`, and revision policy to
`RevisionPolicy::None {}`. A call before 60 seconds reuses the selected output.
At 60 seconds, it checks the origin even within the source TTL. If only another
Git folder changed, it keeps the selected output and reports the new commit as
`checked_source`. A changed selected folder creates a new output; the older one
is eligible for reclaim once its live consumers and other retention demands end.
The local Git fixture in `tests/acquisition_updates.rs` executes this sequence.

Successful check state is a distinct `acquisition` index record. It publishes in
the same journal as a selected managed output and never rewrites that output's
original provenance. Failed checks preserve the previous selection and timestamp;
explicit stale fallback returns the previous selection with `stale: true`.
Requests with different effective update/revision/verification policies have
separate check proofs and may require an initial check; this does not rewrite
source freshness or update intervals. Explicit `new_entry` still creates an output even when
content is unchanged. App-scoped updates use the same check/publication state with separate scope identity.


App-scoped native operations use `session.retrieve_referenced(&request, &catalog, scope, &options)`
and `session.compose_referenced(&retrieval, &acquisition, &catalog, scope, &options)`.
They may create no claim while still enforcing configured source, action and
scope restrictions. A restricted app cannot omit scope to become an administrative
caller. Its unclaimed selection/check state includes app and explicit scope.
Anonymous operations do not synthesize an app.

For claims, reference an `update_policy` definition (or use a registered default)
and call `session.ensure_referenced`. Existing claims require explicit adoption
to change their selected policy; repeated ensure calls do not adopt new defaults. Due checks require the
update action, retain the claim UUID/context, and publish check state with the
selected output and claim. They do not activate binaries or change bindings.
`allow_automatic_updates` and `minimum_update_interval` bound registered update
choices; existing minimum/maximum keep bounds govern revision retention.

Credentials are still independently optional. A session registered with
`credentials_required: false` needs no credential, but its configured permissions
and policy limits still apply. With required credentials, use the existing
credential-bearing session constructor; omitting a claim never omits that proof.

For registered fresh operations, open `RegisteredRetriever::open(root)` instead
of opening/recovering a normal `Store`. Its `retrieve_referenced` method provides trusted
administrative access; `.session(app, credential)?.retrieve_referenced(...)` retains app
restrictions. Both accept only registered temporary/no_store requests. Basic
integrity needs no keyring. Pending metadata transactions return
`recovery_required`; reconcile through a normal store operation separately.
Successful fresh calls leave source caches and catalog contents unchanged.

## CLI composition

`retrieve request.json` returns `{ "request": ..., "descriptor": ... }`.
Use absolute caller destinations when a receipt will be consumed from a different
working directory; relative request paths retain ordinary working-directory semantics.
Cached receipts identify a managed retrieval UUID; no-store receipts describe a
caller-owned file, directory or bare Git repository. Inline fresh commands do not
resolve a home or store, even if `--root` is supplied. Cached and registered-source
commands use the ordinary default `.saucepan` root or an explicit `--root`.

Pass `--policies policies.json` for the portable catalog above. Omit the sidecar
when using persistent definitions: portable UUIDs cannot shadow registered UUIDs.

For a cached receipt, `acquire-retrieved selection.json` accepts:

```json
{
  "retrieval": { "mode": "managed", "uuid": "RETRIEVAL_UUID" },
  "acquisition": { "folder": "plugins/first" }
}
```

For a no-store receipt, set `retrieval.mode` to `caller` and include its `request`
and `descriptor` fields. Set the acquisition destination to a new directory.
`Retriever::open_caller_referenced` is the corresponding native operation. Reopening checks
representation, path, Git objects/dependencies and requested digests; it discards
serialized check/verification evidence. Caller-supplied provenance and expected
digests remain caller assertions, not independently authenticated origin proof.
A caller receipt cannot grant managed ownership or create a managed output.

`source-path RETRIEVAL_UUID` returns an address without fetching or retaining a
lease. `configure-source SOURCE_UUID policies.json` accepts
`{"expected_revision":0,"policy_refs":["REGISTERED_POLICY_UUID"]}`. `purge-retrieval RETRIEVAL_UUID` retires
source material only when no source consumers are live. It preserves acquired
outputs and source registration; Git objects shared with other retrievals remain.

`run-retrieved request.json -- PROGRAM ARG...` retains source material for a child.
It sets `SAUCEPAN_SOURCE_PATH`, `SAUCEPAN_SOURCE_KIND` (`file`, `directory` or
`git_repository`), and, for Git, `SAUCEPAN_SOURCE_COMMIT`. Repository consumers
must treat that path as read-only repository data, not a working checkout.

`run-acquired composition.json -- PROGRAM ARG...` accepts a `retrieval` request
and an `acquisition` request and sets `SAUCEPAN_RESOURCE_DIR` to copied results.
For example, this file fetches a Git repository fresh and copies one folder for
as long as the consumer needs it:

```json
{
  "retrieval": {
    "source": { "kind": "inline", "source": { "type": "github_repo", "repo": "owner/project" } },
    "selector": { "type": "ref", "reference": "main" },
    "policy_refs": ["10000000-0000-4000-8000-000000000001"]
  },
  "acquisition": {
    "destination": { "mode": "temporary" },
    "folder": "plugins/first"
  }
}
```

Arguments are passed literally without a shell; standard streams and child exit
codes are preserved. Inherited native markers protect temporary and managed data
even after wrapper death. Children must preserve those handles. Temporary crash
leftovers are swept by later consumer commands only when native liveness checks
prove the handles closed. A bare temporary path is rejected; use a consumer or a
caller-owned destination. Failed launches clean up their temporary results.

`acquire-retrieved` consumes/releases its source guard before an automatic update;
prior output leases can remain alive. No-store destinations are create-new: use a
new source/output destination in a new composition to fetch another revision.
Configured app restrictions still apply with `--as-app APP_UUID --scope KEY`;
these composition commands create no claim. Use `ensure` for claimed selection.
Credentials are needed only when required by that application's registration.
The older `acquire`, query and application lifecycle commands remain convenience
interfaces. Their `--force-refresh`/`--allow-stale` flags are not accepted by the
new commands, which express these decisions in typed request policies.

`tests/composition_cli.rs` executes these flows using disposable roots and no
home-directory environment, including native child survival and failed launches.
