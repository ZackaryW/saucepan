# Policy resources

The flat policy contract gives each definition its own immutable UUID. Applications,
sources and claims associate those records through `policy_refs`; release selection
uses an ordered `filter_refs` list. Policy definitions never reference other policies.

Implementation status: typed records, catalogs, evaluation, association adoption,
CLI lookup and reference-based composition commands are available. The remaining
legacy operation inputs are being removed under `flatten-policy-resources`.

```json
[
  {"uuid":"10000000-0000-4000-8000-000000000001","type":"access_policy","scopes":["library:main"]},
  {"uuid":"10000000-0000-4000-8000-000000000002","type":"update_policy","mode":"max_age","seconds":86400},
  {"uuid":"10000000-0000-4000-8000-000000000003","type":"revision_policy","history":"latest","keep":3}
]
```

`PolicyCatalog::new` validates the collection and `get(uuid)` returns its complete
typed definition without filesystem or provider access. Unknown IDs, duplicate IDs
and caller catalogs shadowing an existing definition fail. Registration will allow
identical retries, but changing a definition requires a new UUID and explicit adoption.

| Definition type | Target |
| --- | --- |
| `access_policy` | Registered source/action/scope restrictions |
| `source_visibility_policy` | Source discovery |
| `claim_visibility_policy` | Application public claim scopes |
| `cache_policy`, `refresh_policy`, `stale_policy` | Retrieval |
| `verification_policy`, target `payload` | Retrieval digest checks |
| `verification_policy`, target `path` | Path content rechecks |
| `github_release_asset_filter`, `selection_policy` | Release asset selection |
| `identical_pull_policy`, `update_policy`, `revision_policy` | Copied output behavior |
| `reclaim_policy`, `useable_app_policy` | Recovery and active-binding protection |
| `policy_constraints` | Registered permitted choices and bounds |

For new operations, behavior precedence is explicit request, application default,
source default, then built-in default. Existing claims retain their adopted behavior.
All applicable current restrictions still intersect: an omitted allowance adds no
restriction, while an empty allowance permits nothing. Request definitions cannot
grant access. Multiple singleton policies at the same level conflict; list order is
meaningful only for release filters.

Defaults remain cached retrieval, 24-hour freshness, no stale fallback, manual
acquisition updates, output reuse, latest-ten managed history, report-unavailable
recovery, if-available expected-digest checks and metadata-only path access. They do
not create synthetic policy records. Fresh modes bypass reusable caches; managed-only
revision/protection defaults are not applied to unmanaged outputs, and explicitly
requesting those policies for an unmanaged output fails.

The pure evaluator returns derived values and UUID/digest evidence. It does not open
a store, fetch content, authenticate credentials or grant application ownership.
Those checks remain at the operation/session boundary. Integrity mode and optional
application credentials are independent of reusable policy definitions.

Persistent definitions live at `policies/<uuid>.json` inside the existing verified
metadata catalog. `Store::register_policies` publishes a batch atomically; an identical
retry does not advance the catalog. `Store::policy(uuid)` returns the full verified
definition. App-bound lookup additionally requires a readable association and current
optional credentials. A UUID alone does not change caller authority.

`adopt_application_policies` and `adopt_claim_policies` require the expected application
revision; `adopt_source_policies` requires the source's `policy_revision`. Claims keep
their selection and context. Source changes retain provider data and discovery state.
The transition implementation translates references into existing execution settings;
removing those nested public inputs remains part of the pending contract cutover.

Definitions are retained after detachment, reclaim and uninstall so historical
references remain meaningful. There is no definition deletion or overwrite operation.
Journal recovery verifies definitions using the selected basic/keyring mode. Unknown
caller UUIDs report `invalid_request`; missing or corrupt expected catalog members
report `integrity_error` rather than becoming default policies.

Application registration/configuration now accepts `policy_refs`; nested `access`
and `policies` inputs are rejected. Source visibility and freshness convenience
operations publish a new immutable definition and its source association in one
metadata transaction. Source records persist references and attachment revisions;
visibility and freshness are derived when read. Repeating an attached choice is a
no-op, and replaced definitions remain available by UUID.

Save the first JSON array in this guide as `policies.json`. Against a freshly
initialized root, register and inspect a complete definition without an app UUID:

```powershell
saucepan init
saucepan register-policies policies.json
saucepan policy 10000000-0000-4000-8000-000000000003
```

`--root PATH` is optional and overrides the default home `.saucepan` root.
`--as-app UUID` restricts inspection to readable associations, with
`--credential-file PATH` when that application requires proof.

Composition commands (`retrieve`, `acquire-retrieved`, `run-retrieved`,
`run-acquired`) accept reference-only requests. `--policies FILE` supplies an array
of portable definitions for fresh operations. Inline temporary/no-store requests
do not resolve a home or store; registered fresh requests read verified metadata
without recovery or cache access. Portable definitions cannot shadow persistent
UUIDs, even with identical bytes. Register definitions explicitly before managed
publication and omit those definitions from the portable catalog.

Caller receipts contain a reference-only request and a normalized source
descriptor. Reopening validates caller ownership and the material again; serialized
descriptors cannot manufacture managed ownership or a lease. Automatic acquisition
updates use an owned source handle so it can be replaced safely. Borrowed native
acquisition retains its original source guard through the copy operation.
