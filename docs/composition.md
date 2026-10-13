# Retrieval and acquisition contracts

The new request models separate three policy decisions. Native inline fresh
retrieval is available through `Retriever`, and managed retrieval through `Store`.
Offline acquisition is available through `Acquirer`; automatic update orchestration
and new CLI commands are still being integrated.

| Policy | Responsibility | Managed default |
| --- | --- | --- |
| Retrieval cache/freshness | Obtain original source material and decide whether to cache it | Cached, check after 86,400 seconds |
| Acquisition update | Decide when to check whether copied results have changed | Manual |
| Revision history | Decide how many acquired revisions to retain | Latest ten |

`models::retrieval::RetrieveRequest` selects a source, revision and original asset.
Git retrieval returns repository data and an exact commit; a ZIP stays an archive.
Folder selection and extraction belong to `models::store::AcquisitionRequest`.

For example, retrieve a fresh archive into an explicitly caller-owned destination:

```json
{
  "source": {
    "kind": "inline",
    "source": { "type": "url", "url": "https://example.invalid/tool.zip" }
  },
  "selector": { "type": "url" },
  "cache": { "mode": "no_store", "destination": "download/tool.zip" },
  "verification": "if_available"
}
```

Then acquire its files into another caller-owned location:

```json
{
  "destination": { "mode": "directory", "path": "unpacked/tool" },
  "format": "zip",
  "update_policy": { "mode": "manual" }
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

For fresh native retrieval, pass a `RetrieveRequest` to
`core::retrieval::Retriever::retrieve(&request)`. This entry point accepts inline
temporary/no-store requests without opening a store or locating a home directory.
It rejects registered/cached requests that require an owning store. Keep temporary
handles alive while reading their paths. No-store publication transfers ownership
to the caller. Git delivery is a directory containing `repo` and dependencies;
`source_path()` identifies the bare `repo` directory inside it. Downloads and local
files use the exact requested destination filename. Parent directories must exist.

The native example `cargo run --example retrieve-source -- REQUEST.json` inspects
source metadata while retaining the handle. It emits a descriptor only for
caller-owned results; temporary paths are not advertised after their owner exits.

Managed native retrieval uses `store.retrieve(&request)` with cached mode.
It registers an inline source or uses a registered UUID, publishes a retrieval
record under `index/<uuid>/index.json`, and stores original non-Git material under
`source/<source-uuid>/retrieval/<uuid>/material`. Git repository/dependency caches
remain under the source directory. No acquired output or claim is created.
`store.source_path(uuid)` is a bare address; `store.lease_retrieval(uuid)` retains
coordinated access without refreshing. `store.purge_retrieval(uuid)` returns false
while source consumers are live; after they release, retry to retire that record.
Purge does not prune shared Git objects or delete acquired copies or registrations.

`store.configure_source_freshness(uuid, policy)` sets the native retrieval default.
Omitting cached `freshness` inherits it, or the original source TTL when none is
configured. Per-call overrides do not modify the source. Original material has
its own journaled publication and integrity checks, separate from output reclaim.

Only managed outputs accept `revision_policy`. `{"history":"none"}` means keep
the newest revision; `{"history":"latest","keep":3}` includes the newest among
the three. Live leases, active protection and other surviving retention demands
can defer deletion. Existing convenience `policies.eviction` settings translate
to the same retention evaluator. New requests reject that spelling.

Update modes are `manual`, `always`, and `max_age` with `seconds`. A due update
checks the selected content before reuse, even within the retrieval cache TTL.
It never changes an explicit pinned commit. A changed repository head with an
unchanged selected folder may reuse old bytes; checked and selected provenance
remain distinct. Due updates conflict with `cache_only` retrieval.

Payload verification is `none`, `if_available`, or `required`. An explicit digest
cannot be combined with `none`. Required verification needs an expected digest;
a self-computed storage identity is not proof of upstream authenticity. Structural
validation and managed metadata/manifest integrity retain their own contracts.

For offline acquisition, use `Acquirer::acquire(&retrieved, &selection)` with
manual update mode. Managed destinations use the retrieval's owning store;
temporary and directory destinations create independent copies. The source guard
may be dropped after copying. Temporary acquired paths last until their handle
is dropped; directory destinations remain caller-owned.

One repository can supply two subpaths without another provider request:

```rust
use saucepan::core::store::Acquirer;
use saucepan::models::store::{AcquisitionRequest, Destination};

let repository = store.retrieve(&git_request)?;
let source_path = repository.source_path(); // bare repository, exact commit in descriptor
let first = Acquirer::acquire(&repository, &AcquisitionRequest {
    folder: Some("plugins/first".into()),
    destination: Destination::Temporary {},
    ..Default::default()
})?;
let second = Acquirer::acquire(&repository, &AcquisitionRequest {
    folder: Some("plugins/second".into()),
    destination: Destination::Temporary {},
    ..Default::default()
})?;
// first.path() and second.path() are copied directories, independent of source_path.
```

The integration fixture in `tests/acquisition_composition.rs` runs this pattern
with the Git origin removed after retrieval. It also verifies independent output
lifetimes and cached ZIP extraction surviving source-material purge.
