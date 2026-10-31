# Managed setup

This command surface was introduced in manager v0.11.0 with SDKs 0.3.0, both
published. v0.10.0 binaries do not have it; installing an SDK does not add native
commands. Select an exact published release for production installer adoption.

Obtain a matching manager and shim outside the selected store. Run the manager
directly for first setup, passing an exact release tag:

```text
saucepan --root ROOT setup BUILT_SHIM --tag TAG --operation UUID
saucepan --root ROOT setup-status
```

`setup` initializes an absent/empty root in basic mode, or opens an existing
supported root without changing its integrity mode. It registers the official
release source and explicit-update installation policies, then delegates candidate
validation and publication to bootstrap. The self application owns the stable
`bin/saucepan` (`bin/saucepan.exe` on Windows) and its sidecar. Add that bin directory
to PATH yourself. Setup never changes shell profiles, credentials or other apps.

Repeated setup for the installed tag verifies ownership and reuses the installation
without release discovery. Another tag requires explicit self-update. An app-bound
context cannot run administrative setup. Existing low-level init, policy
registration and bootstrap remain available.

## Results and recovery

Setup JSON has `schema_version: 1`, `root`, `operation`, `lifecycle_operation`,
`status`, `changed`, nullable `app`, `claim`, `output`, `launcher_path` and
`installed_version`, plus `backup_paths`, `diagnostics` and nullable `retry_shim`.
Exit 0 means complete; exit 2 carries a recorded noncomplete outcome. Preflight
errors use the normal error channel. No caller should retry a mutation merely
because it lost the response.

Status JSON has `schema_version: 1`, `root`, `state`, nullable `launcher_path`,
`app`, `claim`, `output`, `installed_version`, `operation`, and `diagnostics`.
States are absent, unmanaged, ready, pending and conflict. Successful inspection
exits 0 even when the installation is not ready. Missing roots are not created;
unsupported or corrupt stores remain errors. Inspection neither acquires nor
probes candidates and does not reconcile pending metadata transactions.

Setup intent is integrity-covered at `.staging/setup/UUID/index.json`. It reserves
source/filter and lifecycle identities before dependent effects. Retry pending
setup with the **same operation and inputs**, using the returned durable
`retry_shim` after external scratch is removed. A changed tag, shim digest, root
identity or migration input conflicts. Failed/conflict operations are terminal;
inspect their evidence and explicitly choose a new operation for a new attempt.
Committed publication remains subject to the existing lifecycle reconciliation
rules. Opening the store never resumes setup acquisition or migration.

New setup records add a metadata kind; older binaries may reject such catalogs.
No format or integrity conversion is performed. Use a setup-capable manager for
roots containing these records, and a separate root for older incompatible tools.

## Store contract upgrades

The release after v0.11.0 replaces the `git` source type with `git_repo`, makes
`github_repo` a persisted type and adds `github_raw`, `git_raw` and
`github_source`. Stores record the `saucepan.platform-sources` contract; roots
created by earlier releases (`nested-policy` or `flat-policy-resources`) fail
with `unsupported_store` and are never converted in place. To upgrade a managed
installation, move the old root aside and reinstall into a fresh root with that
release's official installer (`vX.Y.Z` is the new release tag):

```sh
mv ~/.saucepan ~/.saucepan.pre-upgrade
sh ./install.sh --version vX.Y.Z --root ~/.saucepan --operation "$(uuidgen)"
```

A v0.11.0 installation that runs `self-update` to that release completes the
update (its probe only runs `--version`) and then fails every store command with
this guidance until the root is moved aside and reinstalled; release
verification asserts exactly this.

Consumers must register sources with the new types in a paired release; for
example, Powerspec moves from `{"type": "git"}` to `github_repo` or `git_repo`.

## Explicit raw-manager migration

Migration supports an initialized store containing a raw manager at the exact
future launcher path. Run from an external copy of the manager:

```text
saucepan --root ROOT setup BUILT_SHIM --tag TAG --operation UUID --migrate-unmanaged --expected-sha256 SHA256
```

Compute SHA256 over that existing raw file (PowerShell: `Get-FileHash -LiteralPath
PATH -Algorithm SHA256`; Unix: `sha256sum PATH`). The option, digest and explicit
operation identify the file you consent to displace. Filename and version alone
are not ownership. Hard links, symbolic links/reparse points, unknown sidecars,
directories, changed bytes and existing ownership/reservations are refused.

Before displacement, setup persists intent, creates a no-overwrite backup,
preserves permissions, flushes and verifies bytes, and rechecks the source under
launcher coordination. Backups survive success and failure. A busy Windows
executable remains intact: close processes using it and retry the same operation;
setup never terminates those processes.

There is an availability gap between displacement and managed publication. If
installation fails, use the reported backup directly and inspect the result. Do
not restore it over an occupied managed launcher. A new explicit migration attempt
can reuse uniquely matching verified backup evidence from a terminal prior attempt.
Nonempty uninitialized/binary-only roots require separate manual preservation or a
different root; setup will not move their contents or manufacture ownership.

Before migrating a shared path, stop using consumers that still replace it as a
raw binary. An older Powerspec can use an explicitly selected separate compatible
manager until its managed-setup adapter is adopted.

## Explicit version changes

```text
saucepan --root ROOT self-update APP CLAIM --tag NEW_TAG --operation UUID
```

The tag changes only the self claim's release selector. Source, scope, platform
and adopted policies remain unchanged. Candidate validation and revision checks
precede one recoverable publication of the request, claim selection and active
binding. A failed update preserves the prior installation. Omitting `--tag`
retains the stored selector, and setup never implicitly invokes self-update.

## Official installer scripts

Setup-capable releases publish `install.ps1` and `install.sh`, embedded with the
release tag/commit and covered by `SHA256SUMS`. Download the script from the exact
release, verify its checksum, and run it as a file:

```powershell
.\install.ps1 -Version TAG -Root ROOT -Operation UUID
# Migration also requires -MigrateUnmanaged -ExpectedSha256 SHA256
```

```sh
sh ./install.sh --version TAG --root ROOT --operation UUID
# Migration also requires --migrate-unmanaged --expected-sha256 SHA256
```

The Windows installer needs PowerShell 5.1+ and .NET. The Unix installer needs a
POSIX shell, curl, unzip, standard POSIX utilities, and sha256sum or shasum.
Supported native targets match published Windows x64/ARM64/x86, macOS x64/ARM64,
and Linux x64 releases. The installers validate the archive checksum and exact
flat members, stage both executables outside the store, and pass literal inputs
to Rust. They require no Rust, Python, Node, Git or SDK runtime. Missing
prerequisites and setup-incapable managers fail without a raw-copy fallback.
Execution policy is never bypassed. Diagnostics and PATH instructions use stderr;
stdout and exit 0/2 are the manager's setup result. Only owned scratch is cleaned;
durable shim inputs, receipts and backups remain available.

`-TestReleaseBase` / `--test-release-base` and `-TestGithubApi` /
`--test-github-api` are explicit loopback-only fixture seams. Production endpoints
are fixed; no ambient environment variable changes the release provider.

## Verification

Build host binaries with `cargo build --locked --bins`, then run
`python -m unittest discover -s scripts -p test_setup.py -v`.
Fixtures use isolated roots and loopback releases containing those real binaries.
Local Windows results do not establish Unix or native-keyring acceptance.
