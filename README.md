# Saucepan

Saucepan retrieves sources, produces immutable outputs, and manages optional application claims. The manager and shim are Rust binaries; Python and TypeScript SDKs provide typed calls to the manager. Legacy stores and protocols are not migrated.

## Getting started

Build with Rust 1.89 or newer, or obtain the manager and shim from a release. Git and Git LFS must also be installed when required by a repository source.

```powershell
cargo build --locked --bins
cargo run -- init
cargo run -- query-sources
cargo run -- query-sources --type github_release --repo owner/project
```

Managed commands use `%USERPROFILE%\.saucepan` on Windows or `~/.saucepan` on Unix. Use `--root PATH` for another store. Project-local stores are not auto-discovered, and queries do not initialize missing stores. Public discovery needs no app UUID, scope, or query file; sources are private until explicitly published, so an empty result is expected for a new store.

New stores use basic checksum integrity without an OS keyring. Keyring protection and application credentials are separate opt-ins. See [integrity and access](#integrity-and-access).

### Acquire a local resource

Create an `assets` directory containing the files you want to acquire. Save this as `source.json`:

```json
{"type":"local","path":"./assets"}
```

```powershell
cargo run -- register-source source.json
```

Put the returned source UUID in `request.json`:

```json
{"source_uuid":"00000000-0000-4000-8000-000000000001","selector":{"type":"local"},"policy_refs":[]}
```

```powershell
cargo run -- acquire request.json
cargo run -- path OUTPUT_UUID
```

Use the output UUID returned by `acquire` for `path`. Empty policy references use built-in behavior. The returned directory is an address; use a lease or pin when the files must remain available during or after execution.

## Sources and acquisition

Sources can be local files/directories, URLs, Git repositories, GitHub repositories, or GitHub releases. Git selectors support refs, tags and exact commits. Release selection supports platform and asset filters; a release source does not clone a repository. Remote discovery normally refreshes after 24 hours, with policies for other freshness and stale-result behavior.

Retrieval works with original source material; acquisition copies or extracts the selected results. Cached retrieval, acquisition updates and revision history have independent policies. Inline temporary/no-store requests can run without a home, store, app or keyring.

- [Source discovery](docs/source-query.md): public lookup, optional app identity and CLI filters.
- [Retrieval and acquisition](docs/composition.md): request payloads, source paths, copied results and root-free operations.
- [Policy resources](docs/policy-resources.md): immutable UUID definitions, lookup and explicit adoption.
- [Policy profiles](docs/policy-profiles.md): optional starting points, customization and installation policies.

## Application claims and queries

Applications can associate requests with outputs using an explicit scope. Compatible requests in one scope share a claim; separate scopes can update independently. Labels support exact and glob queries. Reading a claim does not refresh its source, and a claim alone does not prevent output reclamation.

- [Claim CLI](docs/claim-cli.md): registration, acquisition, recovery, credentials and JSON errors.
- [Claim policies](docs/claim-policies.md): permissions, covered requests and unavailable-content recovery.
- [Claim queries](docs/claim-query.md): context labels, scope lookup and revision-aware updates.

## Resource lifetime, groups and pins

Use `run` or native resource leases to retain an output while a consumer uses it. Inline group leases open several prepared outputs together. Registered groups save intent and prepare a selected generation atomically. Durable pins retain exact output UUIDs after the updater exits, until explicit release.

For an update with delayed readers: pin A, prepare and pin B, switch consumers, then unpin A after its readers finish. Consumers keep generated or editable state outside immutable store contents.

- [Resource access](docs/resource-access.md): exact paths, verification and native/process lifetimes.
- [Group leases](docs/group-leases.md): named prepared resources under one invocation lifetime.
- [Registered groups](docs/resource-groups.md): registration, preparation and generation launch.
- [Output pins](docs/output-pins.md): explicit durable retention and handover.

## Integrating an application

The SDKs accept request objects and invoke an explicitly selected manager or find it on PATH. Installing an SDK does not install the manager. Rust owns policy evaluation, retrieval, publication and retention.

- [Language SDK overview](sdk/README.md): source installation, context/lifetime choices and development commands.
- [Python SDK](sdk/python/README.md): Python 3.12+ installation and typed usage.
- [TypeScript SDK](sdk/ts/README.md): Node 22+, built-folder/tarball installation and explicit host transports.
- [Rust consumer](docs/scoped-consumer.md): app sessions and native lease ownership.

Plugin hosts can retain immutable paths with pins. Command, hook and service hosts can launch a child under a group lease. The application still owns catalog mapping, activation, language environments and shutdown. Detached services or children that close inherited handles need an explicit retention arrangement. Consumer-shaped tests do not establish that Power Eagle, Zush or PowerSpec have been migrated.

## Integrity and access

Basic checksums detect inconsistency. Optional keyring mode uses HMAC with native OS key custody. Neither mode conceals metadata or replaces filesystem permissions. Source, scope and action restrictions apply independently of optional credentials; public source discovery and public claim exposure are separate settings.

[Integrity and custody](docs/claim-integrity.md) explains store modes, credentials and the local trust boundary. Protected stores fail when their key is unavailable and never silently switch to basic mode.

## Managed applications and releases

The manager can install and update applications, including Saucepan itself. A stable shim launches the application's committed binding. Candidate validation precedes activation; downloading an output alone does not switch the running selection.

- [Application lifecycle](docs/application-lifecycle.md): install, update, rollback, shims, uninstall and recovery.
- [Release builds](docs/releases.md): platform ZIPs, SDK archives, checksums and selective manual CI.

## Development

`src/models/` defines records, matching `src/core/` modules own behavior, `src/cli/` adapts commands, and `src/utils/` contains reusable mechanics. `saucepan-shim` is a separate Rust binary. Dependencies are locked in `Cargo.lock`; the SDKs have no third-party runtime dependencies.

```powershell
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
openspec validate --all --strict
```

SDK test and package commands are in the [SDK guide](sdk/README.md#development). Optional native keyring tests require separate selection; basic-store or injected-key tests do not establish native custody. Hosted checking workflows are selected separately from releases. The original `mock/` installation is historical evidence and is not migrated by tests.

## Specifications and change history

This README is the documentation entry point; the linked guides expand its user-facing sections. [OpenSpec specifications](openspec/specs/README.md) own the synchronized requirements. Active changes under `openspec/changes/` own additions awaiting sync; use `openspec list` to find them. [Archived changes](openspec/changes/archive/README.md) preserve design decisions, dependency reviews, integration assessments and dated verification evidence. Those records describe their checkpoints, not current usage instructions or a second schema to support.
