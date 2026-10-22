# Design

## Context

See `proposal.md` for motivation and repository boundaries. Current evidence at `6a4cd06`:

- `src/core/bootstrap.rs` already fixes the self scope, executable name, version probe and managed install semantics. It consumes a prepared release request and built shim.
- `src/core/lifecycle/managed.rs` reserves both launcher paths before acquiring a candidate, retains protection through validation and records commit intent before publication. `src/core/lifecycle.rs` fingerprints operation inputs. Changing generated source/filter identities on a retry would conflict.
- `src/core/policy/profile.rs` has pure expansion with immutable built-in policy UUIDs. Existing profiles and provider selection can supply setup's request without a second policy engine.
- `Store::create_with_integrity_and_keys` rejects nonempty uninitialized roots. `Store::open` validates the existing contract. CLI routing currently initializes only for `init`; `setup` needs its own authorized branch.
- Shim maintenance rejects unowned occupants and reserves the derived sidecar. A version string or Powerspec cache is not ownership evidence.
- SDK lifecycle decoding recognizes existing action types and exit 0/2. New setup envelopes require deliberate typed handling, not forwarding them through an incompatible decoder.
- `scripts/release.py` validates seven binary ZIPs plus SDK artifacts and generates checksums for an exact set of eleven payloads. Installer scripts must be added to that explicit set and workflow publication.

The installed zmem runtime in this session does not implement the selected Node memory-skill contract. No computed memory verification is claimed; this design is grounded in current source/specs and the user's accepted decisions.

## Goals / Non-Goals

**Goals:** one Rust-owned setup procedure, reproducible first installation, explicit migration, durable retries, truthful inspection, and a small public integration surface for Powerspec.

**Non-Goals:** automatic PATH edits, administrator elevation, store-format conversion, SDK post-install hooks, a general package manager, automatic shared upgrades, moving unrelated files, or changing production stores while developing/testing this change. Standalone managers remain supported.

## Decisions

### 1. A setup feature owns orchestration; lifecycle code still owns activation

Add `src/models/setup.rs`, `src/core/setup.rs` and a focused CLI adapter. Proposed public commands:

```text
saucepan [--root ROOT] setup BUILT_SHIM --tag vX.Y.Z [--operation UUID]
saucepan [--root ROOT] setup-status
saucepan [--root ROOT] setup BUILT_SHIM --tag vX.Y.Z --operation UUID --migrate-unmanaged --expected-sha256 HASH
saucepan [--root ROOT] self-update APP CLAIM --tag vX.Y.Z [--operation UUID]
```

The tag is explicit; no silent latest-release selection or downgrade. Setup uses the official `ZackaryW/saucepan` release source, exact tag, host platform, matching ZIP filter, `explicit-update` for-install expansion, existing self scope and existing probe. Platform mappings cover the existing release matrix and reject unsupported combinations. Tests use the established explicit loopback-provider seam, never a production endpoint override inferred from environment.

`setup-status` returns JSON with `schema_version`, canonical `root`, `state` (`absent`, `unmanaged`, `ready`, `pending`, or `conflict`), nullable `launcher_path`, `app`, `claim`, `output`, `installed_version`, `operation`, and diagnostics. It derives readiness from current verified app/claim/binding/owned-launcher evidence, not merely a setup receipt or version probe. It does not initialize, download, change PATH, resume preparation, or repair files. Missing roots return `absent`; unsupported/corrupt roots remain domain errors. Successful inspection exits 0 even when the reported state is not ready.

Setup returns a versioned JSON result with `status` using existing lifecycle phases, `changed`, canonical `root`, nullable installation identities/launcher/version, a setup `operation`, the linked `lifecycle_operation` when allocated, `backup_paths`, and diagnostics/retry information. Exit 0 means complete, 2 means a recorded noncomplete outcome; preflight errors retain normal errors. Readiness and historical completion remain distinct.

A ready installation at the requested tag is reused without network or new registrations. A different installed tag produces an actionable mismatch without updates; the user chooses self-update. Pending setup returns its original ID and requires an explicit retry. A damaged/missing owned shim is not treated as a fresh installation; direct the caller to existing maintenance/recovery.

Alternative: implement the full sequence in both scripts and SDKs. Rejected because it duplicates state, policy and recovery semantics. The existing low-level bootstrap remains useful and unchanged.

### 2. Persist preparation before registration; reuse exact identities on retry

Once a supported root is available, serialize setup per root with a stable lock. Introduce integrity-covered setup records under `.staging/setup/<operation>/index.json`, integrated with metadata membership/identity validation. Record normalized tag/platform/shim digest, selected root identity, profile expansion, reserved source/filter IDs, linked lifecycle operation ID, preparation phase and migration evidence before the corresponding effects. Include setup records in explicit inspection/retry handling without turning `Store::open` into an installer.

Use deterministic reserved identities within one receipt, not fresh random identities on each retry. Extend the source-registration application layer with a setup-specific reserved-ID operation if necessary; do not expose an unsafe metadata-writing shortcut in scripts. Existing immutable source/policy registration must verify equal content on replay. Initialization remains under its existing reservation contract; failure before a setup record can exist reports initialization/preflight failure rather than inventing an operation result.

Retrying the original `--operation` reloads original inputs and checks equality. Changed tag, shim digest, migration consent/digest or root identity conflicts. Persist durable, digest-verified staging copies of inputs needed after the script exits. Retry diagnostics include the durable shim path usable as BUILT_SHIM with the original flags and operation ID, so external temporary staging is not required to survive. Store-backed receipts use the selected integrity mode and contain no credentials. Uncommitted acquisition/probing resumes only through explicit setup retry; committed publication resumes through the existing linked lifecycle operation. Terminal failed/conflict attempts require a new setup ID after inspection. An explicit new attempt may reuse recorded verified backup evidence; it cannot erase prior receipts or assume missing files prove ownership.

Receipt formats are additive for new binaries. Do not promise older binaries can read newly introduced record kinds; compatibility must be tested and the minimum owning binary documented. No existing root's integrity mode or format discriminator is silently rewritten.

### 3. Explicit migration preserves a verified file without adopting it as a shim

User decision: provide explicit verified migration. Scope it to a supported initialized store containing an unowned regular raw manager at the exact reserved launcher path, with no occupied sidecar or ownership/reservation conflict. Require `--migrate-unmanaged`, `--expected-sha256`, and an operation ID. Reject links, junctions/reparse points, hard-linked candidates, directories, mismatched bytes, invalid probes and unknown sidecars. This is explicit authorization over the specified file, not ownership inferred from `--version`.

Run migration from an externally staged manager, never the executable being displaced. Before removing the original path, persist intent, create a no-overwrite backup under the operation's protected recovery area, flush it and verify its digest. Recheck the source under the established launcher-maintenance coordination. Preserve executable permissions. Only then displace the exact observed source and invoke installation. Record each boundary so process interruption can be classified by source/backup/destination digests. Keep backups on success as well as failure.

An unrelated changed file wins over automatic recovery: leave it untouched and report the conflict. Where removal fails because of a running Windows executable, report pending with the original intact. After displacement, a failed install may leave the launcher unavailable; report that explicitly with the usable backup path and original operation. Do not promise atomic continuity across raw-file migration, auto-restore over a published shim, or delete the backup after a timeout. Low-level shim adoption/replacement contracts remain unchanged.

Nonempty uninitialized roots, including binary-only leftovers, are not silently converted. Report a separate-root/manual-preservation prerequisite; this change's automatic migration targets the initialized layout actually observed on this machine. Powerspec's obsolete cache is left in place and ignored as authority. An explicit override can keep using a separately preserved standalone manager.

### 4. Pin first setup and make changing that pin an explicit managed update

An exact setup tag uses the existing `Selector::Tag`; it must not bootstrap whatever becomes latest after staging. The adopted ZIP filter matches `saucepan-*-<host-target>.zip`: the selector pins the release, while the platform filter stays valid across explicit version changes. Extend self-update with an optional exact `--tag`. Without it, preserve the existing request behavior. With it, plan a replacement selector against the existing self claim's source and unchanged scope/platform/policy refs, validate the candidate, and atomically commit the new request, selected output and active binding under the application's revision fence. Failure leaves all three unchanged. Include the requested tag in operation fingerprints and SDK mapping. Do not implement tag changes by separate update-claim/activate calls.

Alternative: pin setup but leave no supported way to advance its selector, or use latest-stable despite an exact installation request. Both break the intended explicit version contract.

### 5. Thin installers and release packaging

Publish `install.ps1` and `install.sh` from the same tagged commit, with an embedded release tag and checksum coverage. Support explicit version/root/operation arguments, plus explicit migration inputs. For ordinary setup without a caller-supplied operation ID, generate and print one to stderr before invoking the manager; migration requires the explicit caller-supplied ID. The selected version must contain setup; v0.10.0 must report unsupported setup instead of falling back to raw copying. Installers verify the requested archive's exact SHA256SUMS entry, stage both binaries in a unique external temporary directory, preserve executable permissions and call setup with literal arguments. Downloads/diagnostics go to stderr; setup JSON and status pass through unchanged. Cleanup only launcher-owned scratch after durable setup handoff; backups and receipts survive.

Windows baseline: PowerShell 5.1+ and .NET archive APIs; Unix baseline: POSIX shell, curl, unzip and either sha256sum or shasum. Detect missing prerequisites before store effects. Cover only published targets. Do not require Python, Node, Cargo, Git or SDK installation for the installer. Do not introduce a curl-pipe-eval requirement or bypass PowerShell execution policy. Output an explicit PATH instruction after successful setup on stderr.

The scripts share no store policy implementation. Their small platform/download/checksum layer is unavoidable before a manager exists. Reuse standard tools directly: .NET `SHA256.Create().ComputeHash(Stream)` (available even when `Get-FileHash` is missing from the host's module path), `System.IO.Compression.ZipArchive` entry inspection, curl's failure/redirect/time-limit flags, and platform hash tools. Inspect exact flat entries, sizes and duplicates before materializing only manager/shim/LICENSE. Verify release checksums before execution; checksums attest consistency with the release manifest, not independent publisher signatures.

Release packaging adds both scripts to the exact allowlist, checks embedded tag/source consistency, includes their hashes and explicitly uploads them. Existing release assets are not overwritten. Preserve selective/manual CI: building release assets does not claim runtime acceptance or trigger all checking workflows.

API evidence: [Get-FileHash](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.utility/get-filehash), [ZipArchive](https://learn.microsoft.com/en-us/dotnet/api/system.io.compression.ziparchive), [curl options](https://curl.se/docs/manpage.html). Repository evidence: `scripts/release.py`, `scripts/test_release.py`, `.github/workflows/release.yml`.

### 6. SDK and consumer boundaries

Add `Client.setup(built_shim, *, tag, operation=None, migrate_unmanaged=False, expected_sha256=None)`, `setup_status()`, and the optional self-update tag, with equivalent TypeScript methods. Root stays on client configuration. Setup is trusted local administration; app-bound calls fail rather than dropping identity. Installation/import/construction do not download anything. Separate typed setup-result validation from the existing Operation decoder; malformed/inconsistent exits retain mutation uncertainty and no automatic retry.

Powerspec uses the matching immutable SDK, verified installer artifacts, `setup-status` and the returned launcher. It owns compatibility policy and whether setup is authorized, not Saucepan's receipts or shared updates. Development cross-repo tests use an explicit locally built binary and release fixture. Production adoption waits for an actual immutable release containing these interfaces; no guessed release version or unpublished default URL.

### 7. Reuse assessment and verification

No new generic framework is needed. Reuse `utils::tree::digest(Read)` for byte identity, `transfer::copy_bounded(Read, Write, limit, hash)` for bounded copying, `path::plain_metadata`/`beneath` for checked paths, existing lock/process APIs, and `tempfile` staging. These portable contracts already serve ordinary file/export/archive callers. `atomic_write` replaces files and therefore is not suitable for a no-overwrite backup destination; use an exclusively created file in a validated new operation directory, then digest/flush and a feature-owned receipt transition. Setup registration, migration ordering and ownership checks belong to the setup/lifecycle feature, not generic utilities.

Test portable primitives only if a concrete contract changes. Verify new feature behavior through real CLI/SDK calls and isolated files, including crash points before/after backup, displacement, registration, candidate validation and publication, concurrent setup, changed retry inputs, tampered receipts and busy launchers. Reuse `tests/lifecycle.rs`, `tests/managed.rs`, SDK setup tests and `scripts/lifecycle_acceptance.py` patterns. Pack actual built binaries into a loopback release fixture; record hashes, identities, launcher behavior and unchanged prior data. Shell tests include spaces/Unicode, checksum failures, missing prerequisites and unsupported targets. Report platforms/keyring checks actually run.

## Risks / Trade-offs

- Raw migration has a temporary availability gap -> retain a verified executable backup, durable phases and exact retry instructions; do not claim all-or-nothing filesystem rollback.
- Setup records touch integrity/catalog handling -> include corruption, current-store reopen and unrelated app/resource regression checks before integration.
- Scripts need minimal cross-platform duplication -> keep policy and receipts in Rust and exercise both launchers against one versioned contract.
- New provider release is not yet published -> stage local fixture acceptance first; publication and real-home migration require separate explicit actions.
- Shared installations serve multiple consumers -> setup reuses exact versions, and only explicit self-update changes them; consumers check compatibility.

## Migration Plan

1. Implement/verify provider contracts, SDKs and release scripts in isolated roots, preserving all existing low-level commands.
2. Prepare the immutable release artifacts and evidence. Do not publish a tag/release as a side effect of implementation.
3. Apply the linked Powerspec change against those APIs, using local fixtures before an actual release is selected for its production pin.
4. When explicitly requested, run the official installer/migration from external staging against the user's supported initialized root, retaining backup and operation IDs.
5. Roll back managed selections through supported lifecycle commands where compatible. Never restore a raw backup over an owned shim or run an older incompatible binary against newly written metadata; use a separate compatible root/executable if required.
