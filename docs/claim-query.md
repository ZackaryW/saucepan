# Claim context

Context is a plain map of caller-defined string labels on a claim. It never
changes output identity, deduplication, source refresh, or retention. Two projects
can have distinct claims and contexts while sharing the same cached output.

Native callers use `Store::acquire_for_with_context(app, request, context, options)`
to publish a new claim and its labels together. `acquire_for` creates empty
context. Failed acquisition does not publish partial labels. Repeated acquisition
creates another claim; labels do not enforce uniqueness or provide an upsert.

`Store::set_claim_context(app, claim, context, expected_revision)` replaces the
whole map and returns the committed claim. Read the application's revision first.
A stale revision fails, including for a replacement equal to current labels.
An equal replacement at the expected revision is a no-op; `{}` clears labels.
Changed context increments the application's revision without moving its claim or
binding. Missing/removing applications and missing claims fail.

Keys contain 1–128 UTF-8 bytes; values contain 0–4096 bytes; each context has at
most 64 entries. Values must be strings. Dotted keys are literal. Paths, case,
whitespace, and Unicode are preserved: the consumer owns canonical project names.
Omitting context means empty metadata in the current contract, not a general
schema compatibility or legacy migration promise.

Primitive updates preserve context present at commit time, including a concurrent
context edit. Managed updates and rollback preserve labels but conflict if a
context edit changes their captured application revision. Deactivation and
recovery preserve labels. Release/uninstall remove them with the claim; reclaim
keeps them alongside the claim's unavailable output. Managed installation and
bootstrap start with empty context; use explicit replacement to label them.

## Native queries

`core::query` exports `ClaimQuery`, `Condition`, `Operator`, and `QueryResult`.
`Store::query_claims(&query)` reads one application's filtered claims under the
metadata lock. The JSON form is:

```json
{
  "app": "00000000-0000-4000-8000-000000000001",
  "where": [
    {"key": "project", "op": "eq", "value": "C:/work/project-a"},
    {"key": "source.alias", "op": "glob", "value": "skills-*"}
  ]
}
```

All conditions must match. A missing key fails even `glob: "*"`. An explicit
empty `where` returns all claims visible through the app's source filter. Up to
64 conditions are allowed; key/value limits match context. Unknown fields,
operators, missing required fields, and malformed patterns fail even when there
are no candidate claims. A missing/removing/non-application UUID is an error.

Results contain `app`, `revision`, and `claims` (the existing claim/availability
views, now including context). Every match is returned in UUID order, including
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

For consumption, `Store::lease(result.claims[i].claim.entry_uuid, mode)` selects
that exact output; `Store::lease_claim(app, claim, mode)` selects the claim's current
output atomically with lease protection. Search followed by either operation is
not one atomic operation and can fail if reclaim/removal wins. Context and queries
provide neither authentication nor standing ownership of future matches.

## CLI flow

All commands require `--root`. After registering an application and creating a
request as described in the README, use these PowerShell commands. `$root` is
your explicit test store path and `$app` is the registered application's UUID.

```powershell
$a = cargo run -- --root $root acquire request.json --app $app --context examples/claim-query/context-a.json | ConvertFrom-Json
$b = cargo run -- --root $root acquire request.json --app $app --context examples/claim-query/context-b.json | ConvertFrom-Json
$query = Get-Content examples/claim-query/query.json -Raw | ConvertFrom-Json
$query.app = $app
$query | ConvertTo-Json -Depth 8 | Set-Content query.json
$found = cargo run -- --root $root query-claims query.json | ConvertFrom-Json
cargo run -- --root $root set-claim-context $app $a.claim.uuid examples/claim-query/context-b.json --expected-revision $found.revision
cargo run -- --root $root query-claims query.json
```

The first search returns project A's claim. Replacement labels it as project B;
the last search succeeds with no matches. Searching project B then returns both
claims. With identical inputs and reuse policy, their output UUIDs are equal.
An empty context file object clears labels. `--context` requires `--app`.

Successful queries print one JSON object and exit zero, including zero matches.
Invalid input, missing applications, and revision conflicts exit nonzero with
an error on stderr. The JSON query file contains the exact app UUID; there is
no second application argument and no wildcard update/release operation.

`cargo test --test query_cli` runs this acquisition/search/replacement flow using
the example JSON files and a fresh temporary root, also checking native parity,
multiple matches, stale revisions, error exits, and clearing context. It never
uses the existing `mock/` installation or a production Saucepan root.

## Verification evidence

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
