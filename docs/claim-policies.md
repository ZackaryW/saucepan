# Application claim policies

The flat-policy native flow registers standalone definitions with
`Store::register_policies`, attaches app defaults with
`Store::adopt_application_policies`, and calls `AppSession::ensure_referenced`
with `models::claim::request::Ensure`. Its policy input is only `policy_refs`;
asset patterns are selected through `selection.filter_refs`.

Each new claim captures the selected behavior and definition digests. Equivalent
policy UUIDs reuse an exact or covered claim within the same explicit scope,
without replacing its adopted references. Another scope gets its own claim.
Later app defaults apply to new claims; explicit `adopt_claim_policies` changes an
existing claim at its expected application revision. Identity, context, selected
output and binding remain intact. Current access restrictions always apply, even
to cache hits, recovery and historical output replay.

`tests/policy_claims.rs` exercises defaults, coverage, separate scopes, explicit
adoption and automatic updates. `Store::ensure_claim`, `AppSession::ensure` and
the CLI `ensure-claim` command accept the same reference-only `EnsureRequest`.
Caller-supplied `overrides`, `policies` and adopted evidence are rejected.

The Rust contracts live in `models::claim`; `core::claim` owns integrity-verified
persistence, app sessions and the shared evaluator. App claims are stored in a
separate registry. Create an app using `register_application` with an explicit
registration key; retries return the same identity and never redisclose a token.
`Store::session(app, credential)` returns the restricted app-facing API.
`configure_application` is separate local administration and requires a matching
revision plus current proof when enabled. Native and CLI ensure, recovery and
credential delivery are available; see [claim-cli.md](claim-cli.md).

A policy independently restricts actions, source UUIDs, and exact scope strings:

```json
{
  "uuid": "10000000-0000-4000-8000-000000000001",
  "type": "access_policy",
  "actions": ["read", "acquire", "lease", "update"],
  "sources": ["71dd0588-f70d-47e9-93d9-7fd4634dab5e"],
  "scopes": ["project-a"]
}
```

Every configured dimension must permit the operation. An omitted allowance (or
`null`) does not restrict that dimension; an empty array permits nothing.
Scopes compare literally and contain 1–4096 UTF-8 bytes. They are not inferred
from the current directory or from context labels. Context remains searchable
metadata rather than permission or identity.

Credentials prove app identity when enabled. A valid credential does not override
a denied source/action/scope; disabling credentials does not remove restrictions.
Public claim discovery requires explicit public scope exposure and discovery
permission and cannot acquire or mutate a claim. Public source discovery has a
separate per-source visibility flag; app source queries check discovery and source
allowances without requiring a claim scope. See [source discovery](source-query.md).
Changing registered permissions is an explicit
trusted local administration operation, not a grant an app can assign itself.

The evaluator receives a verified caller context from the session boundary. Its
`Proof` and `Caller` inputs are not a credential verifier and must not be populated
by deserializing an app request. Direct Rust administration is a trusted local
API, not a sandbox against programs running as the same OS user.

Registered retention and recovery policy is separate from permissions. Defaults
keep ten entries and report unavailable content. Referenced choices must stay within
registered retention bounds and allowed identical/recovery strategies; they may
not remove required `useable_app` protection. `reclaim_policy` is typed:

```json
{"uuid":"10000000-0000-4000-8000-000000000002","type":"reclaim_policy","strategy":"restore_exact"}
```

`tests/claim_contracts.rs` exercises literal scopes, strict parsing, omitted and
empty permissions, the action matrix, independent proof, public discovery,
policy override bounds, safe view paths, and secret redaction.

## Native errors

Native callers can inspect `saucepan::error::Error::from_anyhow(&error).code`.
Typed causes survive `anyhow` context; no English substring matching is used.
The CLI supports the same codes with `--error-format json`.

| Codes | Meaning |
| --- | --- |
| `invalid_request` | Invalid caller contract |
| `authentication_required`, `authentication_failed` | Missing or rejected proof |
| `access_denied` | Registered operation policy denies access |
| `key_unavailable`, `integrity_error` | Key custody or persisted data verification failure |
| `unknown_application`, `unknown_claim`, `unknown_output` | Missing or wrong-kind identity |
| `content_unavailable`, `exact_content_unavailable` | Unavailable selected or exact historical content |
| `revision_conflict`, `claim_context_conflict`, `claim_overlap`, `policy_conflict` | Conflicting state or intent |
| `source_error`, `io_error` | Acquisition provider or filesystem failure |
| `unsupported_store`, `internal_error` | Unsupported contract or unclassified internal failure |

`tests/errors.rs` executes actual store calls to verify representative codes for
validation, missing identities, reclaimed output, failed source acquisition and
malformed stored JSON. Registry, credential and recovery codes are exercised by
`tests/registry_security.rs`, `tests/claim_cli.rs` and `tests/claim_recovery.rs`.
Unclassified errors use `internal_error`
with a generic message rather than leaking arbitrary error text.

`query_claims` accepts an optional exact `scope`; omission searches permitted
scopes in that one app. `discover_claims` is a separate anonymous native operation
that returns only explicitly public, permitted, credential-free candidates. It
does not create a claim or authorize direct UUID access. Store-level acquisition,
output inspection and maintenance are trusted local administrative APIs; app
integrations must retain their `AppSession` boundary.

`cargo test --offline --test registry_security` exercises enforcement against
actual stores. `cargo test --offline --test lifecycle protected_shim` verifies
unattended protected launch and edit rejection. Cached output UUIDs from discovery
are subject to the same app claim policy when leased through a session.

## Scoped ensure and covered views

`AppSession::ensure(&EnsureRequest, &AcquireOptions)` uses registered defaults
for new claims without explicit policy references. Its explicit `scope` identifies the project independently
of searchable context. With the default `reuse` policy, an exact available retry
returns the existing claim and cached output without refreshing its source.
Another scope gets an independent claim and can
share the same output bytes. Use `update` to refresh a selected claim explicitly.

A natural local/Git claim for the whole tree can cover `folder: "packages/tool"`
in the same scope. The result retains the owning claim UUID and includes
`view.relative: "packages/tool"`; queries show only the owner. Selection constraints
(platform, binaries, format and filters), retention, protection and recovery must
agree, and the directory and requested binaries must exist in the owning manifest.
Folder boundaries are path components: `packages/tool` does not cover
`packages/toolkit`. A later broader request returns `claim_overlap`, as do
incompatible ownership policies. Different context returns `claim_context_conflict`;
replace labels with a revision-checked context operation.

An adopted `new_entry` policy advances the compatible owner to a fresh output without duplicating
the claim. A covered request prepares the entire owning tree and validates the
projection before publication. Concurrent changes are reconciled under the store
lock or return a typed conflict; they are never silently overwritten.

Keep the native guard returned by `session.lease_view(&result.view, mode)` alive
while reading. Its directory is the selected subfolder, but its lock protects the
whole output. A later lease follows the claim's current selection; an already
held lease retains its original bytes. The plain path in an ensure response has
no lifetime protection after the call. Integration examples and concurrent CLI
coverage are executable in `tests/claim_ensure.rs`; registry permissions and public
discovery are covered in `tests/registry_security.rs`.

## Unavailable content

`session.recover(claim_uuid, options)` and ensure of an unavailable existing claim
use its effective `reclaim_policy`. `report_unavailable` returns
`content_unavailable` before any provider work. `restore_exact` uses the saved Git
commit or download URL/asset, validates the complete manifest and dependency
identities, and publishes a fresh output UUID. Changed bytes return `integrity_error`;
unobtainable originals return `exact_content_unavailable`. Neither falls back to a
newer version. `refresh` reruns the owning request with the source's ordinary TTL
and explicit refresh/stale options. It reports whether discovery was refreshed.

Recovery retains scope, context and claim UUID, checks the application revision
before changing selection, and never activates a binding. Another scope sharing
the unavailable output remains unchanged. Queries and ordinary leases never
recover implicitly. Eviction still governs retained history; low-level
`reacquire ENTRY_UUID` reruns a saved request without moving a claim and is not
exact restoration. `tests/claim_recovery.rs` exercises real local/Git/HTTP providers.
