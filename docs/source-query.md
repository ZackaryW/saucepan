# Source discovery

Source discovery finds registered provider definitions before any application has
claimed or downloaded them. It is separate from claim context queries and remote
release/ref discovery. It never fetches, refreshes, reads payloads, creates claims
or retains a lease. Normal store-open recovery still applies.

## Start with a query

```powershell
saucepan query-sources
saucepan query-sources --type github_release --repo owner/project
```

No app UUID, scope, credentials or query file is required for anonymous discovery.
Results include only explicitly public sources. An empty result can mean no
sources have been published; it does not mean you need to register an app.

Commands default to `.saucepan` in your home directory (`%USERPROFILE%` on Windows,
`~` on Unix). If the store does not exist, run `saucepan init` once. Queries fail
with initialization guidance and never create a missing store. A project-local
`.saucepan` directory is not selected automatically. Override the location with
`saucepan --root ./scratch query-sources`; an override never falls back to your
default store. If a valid home cannot be resolved, supply `--root` explicitly.

A trusted local administrator can publish a registered source with
`saucepan set-source-visibility SOURCE_UUID public`, or withdraw it with `private`.
Publication exposes the full source definition; it does not publish private claims.

Use `saucepan --as-app APP_UUID query-sources` only when querying as a registered
application. That app's permissions and optional credentials apply. Failure to
authenticate never falls back to anonymous discovery.

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

All filters are optional and combine with AND. With no filters, `query-sources`
finds every permitted source. Direct flags are `--uuid`, `--type`, `--repo`,
`--origin` and `--url`, with the same meanings as the corresponding JSON fields.
Saved queries remain optional: `saucepan query-sources query.json`, for example:

```json
{"type":"github_release","repo":"owner/project"}
```

Supply either a saved file or direct filter flags. Combining them is an error;
there is no merge or override precedence. Repeated filter flags are also errors.
Global `--root`, `--as-app`, `--credential-file` and `--error-format` options work
with either form. An empty JSON object is equivalent to supplying no query inputs.

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

## Simple application flow

Use a fresh root for this example. Create `source.json` with a local provider
definition pointing to an existing file, for example
`{"type":"local","path":"C:/resources/payload.txt"}`. Create `app.json`:

```json
{"registration_key":"simple-app","name":"Simple App"}
```

For app-owned acquisition, register the app once and query with optional direct
filters. No credentials or scope restriction is needed. This example deliberately
uses a separate `./scratch` root. In PowerShell:

```powershell
saucepan --root ./scratch init
saucepan --root ./scratch register-source source.json
$app = saucepan --root ./scratch register-application app.json | ConvertFrom-Json
$found = saucepan --root ./scratch --as-app $app.uuid query-sources --type local | ConvertFrom-Json
```

Choose a result explicitly; an empty result is valid and multiple results are
returned without choosing one. Put its UUID in `ensure.json`:

```json
{"scope":"default","source_uuid":"SOURCE_UUID","selector":{"type":"local"}}
```

```powershell
$claimed = saucepan --root ./scratch ensure-claim $app.uuid ensure.json | ConvertFrom-Json
saucepan --root ./scratch run-claim $app.uuid $claimed.claim.uuid -- consumer.exe
```

`consumer.exe` reads from `SAUCEPAN_RESOURCE_DIR` while the command retains a
lease. The `default` scope is an explicit ownership label for this simple app;
it does not limit source searching. Different source types need their respective
acquisition selectors. Ensuring again reuses a compatible claim. Querying and
then ensuring are separate operations, so acquisition rechecks current policy.

To expose a source to anonymous discovery, a trusted local administrator runs:

```powershell
saucepan --root ./scratch set-source-visibility SOURCE_UUID public
saucepan --root ./scratch query-sources
saucepan --root ./scratch set-source-visibility SOURCE_UUID private
```

Without `--as-app`, queries use anonymous visibility, not administrative listing.
With `--as-app`, they use that application's allowed sources regardless of public
visibility. Add `--credential-file FILE` only if that app requires credentials.
`set-source-visibility` rejects `--as-app`. `--error-format json` uses the existing
structured error contract; errors exit nonzero without success stdout.

Run `cargo test --offline --test source_query_cli` for the executable fixture,
and `cargo test --offline --lib core::query` for native semantics and refresh
coordination. Fixtures use fresh roots without rewriting historical mock content.
