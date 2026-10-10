# Source discovery

Source discovery finds registered provider definitions before any application has
claimed or downloaded them. It is separate from claim context queries and remote
release/ref discovery. It never fetches, refreshes, reads payloads, creates claims
or retains a lease. Normal store-open recovery still applies.

## Native API

Use `AppSession::query_sources(&SourceQuery)` to find sources permitted for the
application. Required credentials and the current `discover` action are checked,
including for an empty catalogue. Registered source allowances and the existing
application source view filter intersect. Omitted allowances are unrestricted;
an explicitly empty allowance yields no sources. Claim scope restrictions do not
apply to source records, which have no scope. Ensuring a claim still checks its
explicit scope, source and acquire permission.

`Store::query_sources(&SourceQuery)` without an app session performs **anonymous**
discovery and includes only sources with `public_discovery: true`. This does not
depend on any application's public claims. New source records are private by
default. An omitted flag in an authenticated source record also means private;
unsigned legacy stores remain unsupported.

Trusted administration can call `Store::set_source_visibility(uuid, bool)`.
Publishing exposes the full provider definition, including any local path or URL
text: publish only definitions intended for anonymous discovery. This does not
grant acquisition, output access, or visibility of private claims. Registration
retries preserve visibility; updates preserve the UUID and discovery cache and
coordinate with provider refreshes. App sessions cannot configure visibility.

## Query and result

All fields are optional and combine with AND. `{}` finds every permitted source.

```json
{"type":"github_release","repo":"owner/project"}
```

- `uuid`: exact registered source UUID.
- `type`: `local`, `url`, `git`, or `github_release`.
- `repo`: exact stored GitHub release repository string.
- `origin`: exact stored Git origin.
- `url`: exact stored URL source string.

String filters are case-sensitive literal matches, 1..4096 UTF-8 bytes, without
globs, normalization or filesystem traversal. Inapplicable or contradictory
filters match nothing. Unknown fields/types and invalid UUIDs fail. There is no
`scope`, `where`, claim context or arbitrary field dictionary in this query.
`github_repo` registration normalizes to `git`; for example `Owner/Repo` is stored
as `https://github.com/owner/repo.git` and queried using `origin`.

Results contain the observed catalogue `generation` and a `sources` array sorted
by UUID. Each view includes `uuid`, `source` and `public_discovery`. Cached release
assets are not returned. Source metadata is authenticated, including hidden source
records; missing or tampered catalogue members fail rather than disappear from
results. Returned views are snapshots, not ownership grants or leases. Ensure a
claim and retain a lease before reading acquired content.
