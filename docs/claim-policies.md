# Application claim policies

The Rust contracts live in `models::claim`; `core::claim` owns authenticated
persistence, app sessions and the shared evaluator. App claims are stored in a
separate registry. Create an app using `register_application` with an explicit
registration key; retries return the same identity and never redisclose a token.
`Store::session(app, credential)` returns the restricted app-facing API.
`configure_application` is separate local administration and requires a matching
revision plus current proof when enabled. CLI credential input/output and the
remaining ensure/recovery flows are still in progress.

A policy independently restricts actions, source UUIDs, and exact scope strings:

```json
{
  "actions": ["read", "acquire", "lease", "update"],
  "sources": ["71dd0588-f70d-47e9-93d9-7fd4634dab5e"],
  "scopes": ["project-a"],
  "public_scopes": []
}
```

Every configured dimension must permit the operation. An omitted allowance (or
`null`) does not restrict that dimension; an empty array permits nothing.
Scopes compare literally and contain 1–4096 UTF-8 bytes. They are not inferred
from the current directory or from context labels. Context remains searchable
metadata rather than permission or identity.

Credentials prove app identity when enabled. A valid credential does not override
a denied source/action/scope; disabling credentials does not remove restrictions.
Public discovery requires explicit public exposure and discovery permission and
cannot acquire or mutate a claim. Changing registered permissions is an explicit
trusted local administration operation, not a grant an app can assign itself.

The evaluator receives a verified caller context from the session boundary. Its
`Proof` and `Caller` inputs are not a credential verifier and must not be populated
by deserializing an app request. Direct Rust administration is a trusted local
API, not a sandbox against programs running as the same OS user.

Registered retention and recovery policy is separate from permissions. Defaults
keep ten entries and report unavailable content. Claim overrides must stay within
registered retention bounds and allowed identical/recovery strategies; they may
not remove required `useable_app` protection. `reclaim_policy` is typed:

```json
{"type":"reclaim_policy","strategy":"restore_exact"}
```

`tests/claim_contracts.rs` exercises literal scopes, strict parsing, omitted and
empty permissions, the action matrix, independent proof, public discovery,
policy override bounds, safe view paths, and secret redaction.

## Native errors

Native callers can inspect `saucepan::error::Error::from_anyhow(&error).code`.
Typed causes survive `anyhow` context; no English substring matching is used.
The CLI presentation is unchanged in this checkpoint.

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
malformed stored JSON. New registry/auth/recovery-specific codes become reachable
when those operations are integrated. Unclassified errors use `internal_error`
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
