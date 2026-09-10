# Saucepan

Saucepan acquires content from Git repositories, HTTP(S) downloads, and local paths into one user-level store. Applications share source content while keeping their settings and touched-entry views in a central encrypted index.

This checkout implements the new central-store API. The older workspace/TOML commands and Python integrations are not compatible with this API.

## Build and place the executable

Build from this checkout with a current Rust toolchain:

```sh
cargo build --release --locked
mkdir -p "$HOME/.saucepan/bin"
cp target/release/saucepan "$HOME/.saucepan/bin/saucepan"
```

PowerShell:

```powershell
cargo build --release --locked
New-Item -ItemType Directory -Force "$HOME/.saucepan/bin"
Copy-Item target/release/saucepan.exe "$HOME/.saucepan/bin/saucepan.exe"
```

Add that `bin` directory to PATH. One active executable serves all apps. Git acquisitions require Git; repositories using Git LFS also require Git LFS.

## First acquisition

Initialize the encrypted store once, then register an app:

```sh
saucepan init
saucepan register example-app > .saucepanhash
```

The key lives in Windows Credential Store, macOS Keychain, or Linux Secret Service. An unavailable native service is an error. Settings live in the encrypted index; no `saucepan.toml` is read.

Save this as `recipe.json` (replace the path with a directory you own):

```json
{"source":{"provider":"local","path":"./assets"}}
```

```sh
saucepan --marker .saucepanhash acquire recipe.json
saucepan --marker .saucepanhash view
```

Acquisition returns JSON with an artifact record and its central directory. The marker is a stable caller token; it does not need rewriting when settings or acquired entries change.

Snapshots default on, repeated content verification defaults off, and remote-failure cache fallback defaults off. Each source/ref keeps one current snapshot and up to five historical ZIPs. Git folders share the same source history, and Git exports exclude `.git` at every depth.

See the [central-store guide](docs/central-source-store.md) for recipes, app settings, mirrors, verification, the Rust API, and isolated test mode.

## SDKs

Use the [TypeScript SDK](sdk/typescript/README.md) from Node.js or the [shell SDK](sdk/shell/README.md) from POSIX shells. Both cover the current central-store commands and use an independently installed executable, defaulting to `~/.saucepan/bin/saucepan[.exe]`. App settings remain in the encrypted index.

The [SDK overview](sdk/README.md) records compatibility and installation options. The legacy Python `Workspace` API still requires migration.

## Development

```sh
cargo test --locked
cargo fmt -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
```

Fresh tests live beside the implementation under `src/`. Old root integration tests and local `src2`/`src3` references are excluded from targets and packages. Native keyring tests are explicitly selected because they need an available user credential service. Platform results are recorded in the guide.
