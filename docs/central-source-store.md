# Central source store

An app registers with one Saucepan store at `~/.saucepan`. Registration saves its settings and initial filters in the encrypted index and returns a stable caller token. Successful acquisitions record the entries that app touched. Its view contains those touched entries matching its filters. Filters select records; they do not grant or deny acquisition.

Ordinary calls use `--app example-app`. Authoritative calls use `--marker .saucepanhash`, which loads the token returned by registration and verifies its app/store binding. Adding `--authoritative` without a token fails. The OS user is trusted; this API does not isolate mutually hostile processes running as that same user.

## Recipes

Save a recipe as JSON and pass its filename to `acquire`.

Git, tracking a branch and selecting a folder:

```json
{
  "source": {
    "provider": "git",
    "origin": "https://github.com/example/assets.git",
    "reference": "main"
  },
  "folder": "themes/dark"
}
```

The origin is illustrative; use your actual repository. Add a top-level `commit` containing a full commit ID to acquire an exact revision without advancing the tracked current. Branch/ref is part of source identity; `folder` is not. The original and effective origin of the central Git repository are checked against the recorded identity on Git access.

HTTP(S) file:

```json
{
  "source": {
    "provider": "url",
    "url": "https://example.com/assets/model.bin",
    "download": {"format": "file", "name": "model.bin"}
  }
}
```

HTTP(S) ZIP:

```json
{
  "source": {
    "provider": "url",
    "url": "https://example.com/assets.zip",
    "download": {"format": "zip"}
  },
  "folder": "assets/themes"
}
```

Only ZIP archives with supported Stored/Deflate entries are currently accepted. Folder paths refer to actual archive paths; no automatic GitHub wrapper-folder stripping is assumed. Other archive formats fail explicitly.

Local directory or file:

```json
{"source":{"provider":"local","path":"./assets"},"folder":"themes"}
```

Omit `folder` to use the entire directory, or when acquiring a single file. Local inputs are copied; returned paths always point into central storage. On Windows use JSON forward slashes or escaped backslashes.

Acquisition output includes `artifact`, `directory`, `fallback`, `update_checked`, and `content_verified`. `directory` is always a directory. A file download appears inside it under its declared filename. Exact Git pins and snapshot reads report `update_checked: false`; fallback additionally reports `fallback: true` and preserves the actual cached revision.

## App settings and filters

The defaults are:

```json
{
  "retain_snapshots": true,
  "verify_content": false,
  "allow_local_fallback": false
}
```

Save all three fields in `settings.json` and apply them:

```sh
saucepan --marker .saucepanhash configure --settings settings.json
```

Settings are input to this command, then persisted centrally. They are not reread on acquisition and no app-local TOML overrides them. Changing one app leaves other app settings unchanged.

Filters contain two sets, intersected when both are nonempty:

```json
{"source_ids":[],"providers":["git","local"]}
```

Empty sets match all. `source_ids` contains canonical lowercase SHA-256 source IDs returned by acquisition. Apply the JSON using `configure --filters filters.json`, or use `register example-app --settings settings.json --filters filters.json` during registration. An untouched entry never appears merely because another app cached it. Listing does not create touches.

Repeated content verification compares consumed content with its authenticated manifest. Encryption, proof checks, Git origin consistency, safe export/extraction, and required Git LFS object verification remain mandatory. A failed remote refresh returns an error unless the calling app explicitly enables local fallback and a suitable recorded local copy remains available. Integrity/export errors are not remote failures eligible for fallback.

## Current content, history, and mirrors

Each source identifier owns one current snapshot and up to five historical ZIPs. A successful changed tracking acquisition rolls a retained outgoing current into history, then selects the new current. Historical eviction uses least recent use, then creation order. Reading history or reacquiring an exact retained pin refreshes recency. Reacquiring unchanged content does not add a historical duplicate.

Turning retention off does not erase shared history. Turning it back on saves the next acquisition, even if that current content has not changed; previously unsaved revisions remain gaps. Failed refresh with allowed fallback does not advance current or add/evict history.

Git ZIPs contain the complete exported tree, with exact required submodule and LFS contents. `.git` directories and gitfiles are omitted at every depth. Missing required content causes an explicit error. Original Git/ZIP executable metadata is recorded even on Windows, where the filesystem has no Unix executable bit.

Safe relative symlinks committed in Git are exported as independent ordinary files or directories containing their resolved target bytes. Resolution happens against the complete source tree before folder selection, including exact submodules and validated LFS content. For example, selecting `sdk/` where `sdk/config` links to `../shared/config` returns the resolved content at `config`; the target need not be inside `sdk/`. A directory alias can itself be selected. Editing an exported alias does not change its target or a mirror.

Absolute/drive/UNC targets, backslashes, malformed targets, root escape, broken links, cycles, `.git` targets, and unsupported special files fail explicitly. Expansion permits at most 64 link hops per output path, 1,000,000 expanded entries, and 16 GiB of file bytes, counting alias copies. These checks also apply with retention or repeated content verification disabled. Failed link export does not publish new current/history or app touches and does not qualify for remote-failure fallback. Existing filename and collision checks remain in force.

This support applies to committed Git links. Local filesystem symlinks/reparse points and downloaded ZIP symlink entries remain rejected. Saucepan does not create native symlinks or hardlinks, require symlink privileges, or add a link-related app setting. Snapshots and mirrors contain ordinary content on every supported OS.

```sh
saucepan --marker .saucepanhash history SOURCE_ID
saucepan --marker .saucepanhash snapshot SOURCE_ID SNAPSHOT_ID --folder themes
saucepan --marker .saucepanhash path ARTIFACT_ID
saucepan --marker .saucepanhash mirror ARTIFACT_ID ./exported-themes
```

Replace the uppercase IDs with values from acquisition/history output. Snapshot reads do not advance the source. Mirrors are independent copies at a new destination; occupied destinations and their edits are preserved. Evicting historical ZIPs does not remove live central content directories or mirrors.

Actual layout:

```text
~/.saucepan/
  bin/saucepan[.exe]
  index.json.enc
  index.lock
  sources/<source-id>/
    repo/                         # Git only
    submodules/                   # required Git submodule repositories
    indexes/<state-digest>.json.enc
    snapshots/<snapshot-id>.zip
    content/<snapshot-id>/
```

Current/history are pointers in the authenticated index. Source metadata fragments and content are prepared before the central index is atomically replaced under a lock. Failure leaves the old selection intact. Unselected prepared files and old metadata fragments can remain on disk; the five-entry limit describes selected historical ZIPs, not a total disk quota. No general cleanup or recovery journal is implemented.

## Stable tokens and current-view verification

```sh
saucepan --marker .saucepanhash view > view.json
saucepan --marker .saucepanhash verify view.json
```

Saucepan verifies the supplied view against the current HMAC stored in its encrypted index. Changed settings/scopes/data invalidate a saved old view while the caller token stays valid. Encryption, token binding, and view authentication use separate key domains. The marker carries neither the master key nor the changing data HMAC. It is not an RSA key. Unsupported format versions and future authority restrictions fail explicitly; read-only roles and delegated token issuance are not implemented.

## Native API and explicit test mode

The [Python](../sdk/python/README.md), [TypeScript](../sdk/typescript/README.md), and [POSIX shell](../sdk/shell/README.md) SDKs expose the same central-store operations through the CLI. They use the shared executable or an explicit binary path, support stable markers and isolated test stores, and leave settings and policy in the core. Python 0.5.0 exposes `Saucepan`; callers of the former `Workspace` API must update their integration.

The CLI translates calls into the public Rust API:

```rust,no_run
use saucepan::core::{Store, models::*};

fn example() -> anyhow::Result<()> {
    let store = Store::open_user()?; // create_user() only for first initialization
    let proof = store.register("example-app", AppSettings::default(), Filters::default())?;
    let app = AppContext::authenticated(proof);
    let result = store.acquire(&app, &Recipe {
        source: Source::Local { path: "./assets".into() },
        folder: None,
        commit: None,
    })?;
    let view = store.view(&app)?;
    store.verify_view(&app, &view)?;
    println!("{}", result.directory.display());
    Ok(())
}
```

For isolated tests, call `Store::create_test(custom_root, [test_byte; 32])` and `Store::open_test(custom_root, key)`. CLI equivalents require both `--test-root PATH` and `--test-key HEX64` on every invocation. These explicitly supplied test keys never become automatic production fallbacks.

Use a short custom root on Windows. Deep pytest-generated roots failed during Git acquisition when source and snapshot identifiers lengthened the paths; the Python CLI fixtures use short temporary roots. Arbitrarily long Windows store paths are not validated.

Production initialization creates a fresh native secret and encrypted index. A preexisting `bin` directory is allowed so the executable can be placed first. Existing store data or an existing secret prevents reinitialization; opening with a missing key/index fails without replacement. Native credential failures remain errors on every supported OS.

## Verification recorded on 2026-09-09

Fresh behavior tests were written and observed failing before implementation, followed by regression fixes. The suite covers helper composition, all three providers, Git refs/pins/submodules/LFS, scoped views, HMAC failures, LRU and retention gaps, fallback, independent mirrors, concurrent callers, and failed publication/retry.

Windows: 69 tests passed, one native test excluded from the default suite; that native Credential Store test passed when selected separately. Linux (Rust 1.97.1 on Debian bookworm): 68 tests passed, one native test excluded by default; the native Secret Service test passed in an isolated D-Bus session. The difference is platform-specific filesystem tests. Both also passed the doctest, Clippy with warnings denied, and build checks. Linux without a credential service failed explicitly and created no store.

macOS Keychain support is implemented through the native keyring backend, but no macOS execution result is available from this environment. A focused three-OS workflow is prepared; this is not a claim that its macOS job has run. The earlier root integration tests, SDK fixtures, and reference builds are not evidence for this rewrite.

On 2026-09-10 the two Python central-store CLI fixtures were revised to the current protocol and passed alongside the SDK vendoring test (three tests total). They exercise explicit test keys, Git acquisition, stable markers, and scoped views. That fixture-only update did not migrate the Python runtime; the subsequent client migration is documented below.

Also on 2026-09-10, the new TypeScript SDK passed five real-CLI integration tests and declaration checks on Windows (Node.js 20) and Linux (Node.js 22). These cover all SDK operations, Git/URL/local input, shared Git folder identity, scoped views, proof failures, literal arguments, concurrent request files, and timeout handling. The shell suite exercised every wrapper on Windows Git Bash and Linux dash. Test roots and keys were explicitly isolated from production keyrings. The packed npm artifact passed installation, ESM import, declaration compilation, and an actual CLI call in a separate Windows consumer project. A three-OS SDK workflow is configured; macOS execution remains unverified here.

The six remaining root Rust test files were subsequently revised from the rejected API to the current public contracts and enabled through Cargo's normal integration-test discovery. On Windows, all 10 integration tests passed alongside the 69 unit tests and one doctest; Clippy passed with warnings denied. The root suite covers public JSON schemas, core dependency boundaries, explicit test stores, app filtering, stable markers, and actual binary exit behavior. It is now included in the crate package; local `src2`/`src3` references remain excluded.

Safe Git symlink export was then verified with 80 passing unit tests on Windows and 79 on Linux, plus 10 integration tests and one doctest on each. The native credential test remained excluded from these runs. Formatting and strict Clippy passed; TypeScript's five integration cases and the shell SDK's two local/Git cases passed on Windows and Linux. The existing macOS workflow includes the tests, but no macOS execution result was available. A separate check confirmed that a store and link-free snapshot created by the previous binary retained its token, artifact/snapshot identifiers, and ZIP bytes under the updated binary.

The rebuilt release executable successfully acquired `github/gitignore` over HTTPS at commit `9e86bc12f67365b8dd974d3b3f09d166265c5530`, including its `Global` directory. `Clojure.gitignore`, `Fortran.gitignore`, and `Global/Octave.gitignore` matched their committed targets as ordinary files. ZIP bytes and entry types, shared snapshot reuse, mirrors, recorded origin, and app-view verification passed. This network acceptance check is separate from deterministic automated tests; private credentials and SSH were not exercised.

Python SDK 0.5.0 was subsequently migrated to the current CLI. The first public-client test run failed because `Saucepan` did not exist; after implementation, all 17 Python tests and both BDD scenarios passed on Windows Python 3.9 and Linux Python 3.11. The suite also passed on Windows Python 3.14. These tests now exercise SDK methods for registration, configuration, all three providers, Git aliases/shared folders/history/pins, view verification, mirrors, nullable lookups, concurrency, and process failures. The built wheel was installed in a separate Python 3.9 environment and used for a real acquisition and view verification. The three-OS SDK workflow now includes Python; macOS execution remains unverified here.
