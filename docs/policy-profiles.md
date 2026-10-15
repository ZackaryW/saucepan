# Policy profiles

[← Sources and acquisition](../README.md#sources-and-acquisition)

Profiles are optional starting points. They expand to ordinary immutable policy
records and UUID references. Preview does not open a store or register anything:

```powershell
saucepan profiles
$profile = saucepan profile managed | ConvertFrom-Json
$profile.catalog | ConvertTo-Json -Depth 30 | Set-Content -Encoding utf8 policies.json
saucepan register-policies policies.json
```

Use `retrieval_policy_refs` for retrieve requests, `acquisition_policy_refs` for
copied-result requests, and `combined_policy_refs` for managed acquisition or
application/source defaults. Register records before persistent use. For root-free
temporary operations pass the catalog with `--policies policies.json` instead.
Do not pass a portable catalog that duplicates already registered UUIDs.

| Setting | managed | explicit-update | temporary |
| --- | --- | --- | --- |
| Original material | Cached | Cached | Temporary |
| Source check | After 24 hours | On explicit retrieval | Every retrieval |
| Acquisition updates | Manual | Manual | Unspecified |
| Identical content | Reuse | Reuse | Unspecified |
| Managed history | Latest 10 | Latest 10 | Unspecified |
| Unavailable content | Report unavailable | Report unavailable | Unspecified |

All three disable stale fallback. Neither managed profile schedules updates or
refreshes on lookup. The history window counts the newest ten revisions including
the current revision; older protected entries may survive additionally. Other
surviving retention demands still apply. Profiles never create or release pins.
Verification, credentials, store integrity, access, visibility and scope remain
independent choices. Omitting a profile entirely preserves normal defaults.

For installation use `saucepan profile explicit-update --for-install`. This adds
the ordinary `useable_app_policy`; installation still requires source, executable,
scope, probe and shim inputs. Temporary is not an installation profile.

Override a recipe axis with a standalone policy definition, not an inline settings
bundle:

```powershell
saucepan profile managed --catalog custom-policies.json --acquisition-policy REVISION_POLICY_UUID
```

The returned refs replace that recipe's revision choice. Multiple explicit
choices for one axis are errors. Registered restrictions still apply at execution.
Registering a changed expansion does not change existing claims; use explicit
revision-checked [policy adoption](policy-resources.md). Retain expanded inputs and
operation IDs when retrying setup; a recipe name alone is not a stable receipt.

For temporary material, explicitly choose a temporary or caller-owned acquisition
destination. Use [guarded execution](composition.md) to keep native temporary
material alive. For example, put the expansion's retrieval refs into a retrieval
request for an inline source and run:

```powershell
saucepan --policies policies.json run-retrieved retrieval.json -- your-consumer
```

The consumer receives the existing native resource environment. A temporary path
does not remain valid after its owner exits. Use caller-owned delivery when the
copied results must outlive the operation; use a no-store cache policy and explicit
destination when the original material itself should be caller-owned.
