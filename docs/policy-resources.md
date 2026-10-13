# Policy resources

The flat policy contract gives each definition its own immutable UUID. Applications,
sources and claims associate those records through `policy_refs`; release selection
uses an ordered `filter_refs` list. Policy definitions never reference other policies.

Implementation status: the typed records, in-memory catalog and pure evaluator are
available. Persistent registration, CLI lookup and operation integration are being
implemented under `flatten-policy-resources`; the examples below are contract examples,
not commands supported by the current CLI.

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
