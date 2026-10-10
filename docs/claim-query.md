# Claim context

Context is a plain map of caller-defined string labels on a claim. It never
changes output identity, deduplication, source refresh, or retention. Two projects
can have distinct claims and contexts while sharing the same cached output.

Native app callers use `session.ensure(&EnsureRequest, &AcquireOptions)` to apply
registered defaults, or `session.acquire(scope, request, context, options)` for a
complete acquisition request. The lower-level
`Store::acquire_for_with_context(app, scope, request, context, options)` is also
available without caller proof; `acquire_for` supplies empty context. Those
app-specific Store calls still enforce policies and fail if credentials are
required. Failed acquisition does not publish partial labels. A compatible retry
in the same explicit scope returns the existing claim; changed context requires
revision-checked replacement. Different scopes retain independent ownership.

`session.set_context(claim, context, expected_revision)` replaces the
whole map and returns the committed claim. Read the application's revision first.
A stale revision fails, including for a replacement equal to current labels.
An equal replacement at the expected revision is a no-op; `{}` clears labels.
Changed context increments the application's revision without moving its claim or
binding. Missing/removing applications and missing claims fail.

Keys contain 1–128 UTF-8 bytes; values contain 0–4096 bytes; each context has at
most 64 entries. Values must be strings. Dotted keys are literal. Paths, case,
whitespace, and Unicode are preserved: the consumer owns canonical project names.
Omitting context on an acquisition request means empty metadata. Persisted integrity-verified
claim members require their context field; no older-record decoder or migration
promise is implied.

Primitive updates preserve context present at commit time, including a concurrent
context edit. Managed updates and rollback preserve labels but conflict if a
context edit changes their captured application revision. Deactivation and
recovery preserve labels. Release/uninstall remove them with the claim; reclaim
keeps them alongside the claim's unavailable output. Managed installation and
bootstrap start with empty context; use explicit replacement to label them.

## Native queries

`core::query` exports `ClaimQuery`, `Condition`, `Operator`, and `QueryResult`.
Create `Store::session(app, optional_credential)` and call `session.query(&query)`
to read that application's permitted, filtered claims under the metadata lock.
The query's app UUID must match the session. Direct `Store::query_claims` is
available for credential-free apps and still applies their policies. The JSON
form is:

```json
{
  "app": "00000000-0000-4000-8000-000000000001",
  "scope": "project-a",
  "where": [
    {"key": "project", "op": "eq", "value": "C:/work/project-a"},
    {"key": "source.alias", "op": "glob", "value": "skills-*"}
  ]
}
```

All conditions must match. A missing key fails even `glob: "*"`. An explicit
empty `where` returns all claims permitted by the app's access policies, optional
exact scope and source view filter. Omit `scope` to search all permitted scopes
within that app. Credentials are checked even when no claims match. Up to
64 conditions are allowed; key/value limits match context. Unknown fields,
operators, missing required fields, and malformed patterns fail even when there
are no candidate claims. A missing/removing/non-application UUID is an error.

Results contain `app`, `revision`, and `claims` (the existing claim/availability
views, including each claim's scope and context). Every match is returned in UUID order, including
duplicate labels and unavailable outputs. Zero matches succeeds with an empty
array. Referenced output metadata corruption fails the query. Results do not
automatically follow later edits.

`eq` compares literal complete strings, including `*`. `glob` matches the complete
string with `*` (zero or more Unicode scalars), `?` (one scalar), and backslash
escapes for `*?\\[]{}`. Unescaped brackets/braces and unsupported/trailing escapes
fail. Repeated stars act as one. Separators and newlines are ordinary characters;
Windows and Unix use the same case-sensitive semantics. JSON requires doubling
backslashes: `"skills-\\*"` matches a literal star. No shell or filesystem expansion
occurs, and Unicode is not normalized.

Queries read metadata without refreshing sources, checking payload integrity,
changing revisions, or holding leases. Existing pending transaction recovery
still runs when opening/locking the store. A result's application revision tracks
application mutations, not all output availability changes.

For app consumption, `session.lease_output(result.claims[i].claim.entry_uuid, mode)` selects
that exact authorized output; `session.lease_claim(claim, mode)` selects the claim's current
output atomically with lease protection. Search followed by either operation is
not one atomic operation and can fail if reclaim/removal wins. Context and queries
provide neither authentication nor standing ownership of future matches.

## CLI flow

Commands default to the user-home `.saucepan` store. After registering an application and creating a
request as described in the README, use these PowerShell commands. `$root` is
your explicit test store path and `$app` is the registered application's UUID.

```powershell
$a = cargo run -- --root $root acquire request.json --app $app --scope project-a --context examples/claim-query/context-a.json | ConvertFrom-Json
$b = cargo run -- --root $root acquire request.json --app $app --scope project-b --context examples/claim-query/context-b.json | ConvertFrom-Json
$query = Get-Content examples/claim-query/query.json -Raw | ConvertFrom-Json
$query.app = $app
$query | ConvertTo-Json -Depth 8 | Set-Content query.json
$found = cargo run -- --root $root query-claims query.json | ConvertFrom-Json
cargo run -- --root $root set-claim-context $app $a.claim.uuid examples/claim-query/context-b.json --expected-revision $found.revision
cargo run -- --root $root query-claims query.json
```

The example query file omits `scope`, so it searches both permitted scopes.
The first search returns project A's claim. Replacement labels it as project B;
the last search succeeds with no matches. Searching project B then returns both
claims. With identical inputs and reuse policy, their output UUIDs are equal.
An empty context file object clears labels. `--context` requires `--app`.

Successful queries print one JSON object and exit zero, including zero matches.
Invalid input, missing applications, and revision conflicts exit nonzero with
an error on stderr. The JSON query file contains the exact app UUID; there is
no second application argument and no wildcard update/release operation.

For a credential-protected app, add `--credential-file FILE`. An optional
`--as-app` must agree with the query's app UUID. Anonymous `discover-claims FILE`
instead accepts `{"where":[]}` with an optional exact `scope` and returns only
explicitly public, permitted, credential-free claims. It neither publishes
source definitions nor grants acquisition rights; [source queries](source-query.md)
have a separate visibility setting and do not filter by claim scope.

`cargo test --test query_cli` runs this acquisition/search/replacement flow using
the example JSON files and a fresh temporary root, also checking native parity,
multiple matches, stale revisions, error exits, and clearing context. It never
uses the existing `mock/` installation or a production Saucepan root.

## Earlier query checkpoint evidence

The counts below describe the query checkpoint before the separate claim registry.
Current integrated evidence is in [claim-verification.md](claim-verification.md).

Verified on 2026-10-09 on Windows x86_64 with Rust 1.98.1:

- `cargo test --all-targets`: 96 top-level tests passed (97 reported passes
  including a nested child fixture run). Coverage includes context publication,
  replacement, recovery, update/rollback conflicts, query filtering and snapshots,
  portable string matching, and the CLI example flow.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: passed.
- Strict OpenSpec validation: this change and all 13 existing main specs passed.
- SHA-256 comparison: all 96 existing `mock/` files remained unchanged.

Linux/macOS execution and the declared minimum Rust 1.89 toolchain were not run
for this change. String-matching semantics are specified portably and covered by
platform-neutral cases, but this run establishes Windows execution evidence only.
