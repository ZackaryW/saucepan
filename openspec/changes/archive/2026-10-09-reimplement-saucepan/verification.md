# Implementation verification

Verified on 2026-10-08 with Windows x86_64, Rust 1.98.1, Git 2.55, and Git LFS 3.7.1. The package declares Rust 1.89; locked dependencies' declared MSRVs were reviewed, but execution with the minimum toolchain was not performed.

## Results

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test --all-targets`: 39 tests passed, including process fixture entrypoints.
- `cargo build --all-targets`: passed.
- `openspec validate reimplement-saucepan --strict`: passed.
- `uv run --no-project python scripts/verify_windows_console.py`: passed; explicitly observed `Windows console CTRL_BREAK forwarding verified`.
- `uv run --no-project python scripts/acceptance.py`: passed against an existing compatible mock, preserving prior contents; first run also exercised bootstrap from an empty mock.
- The documented mock shim/version/inspect/path commands ran successfully after the fixture server stopped.

Linux cross-check was attempted with `cargo check --target x86_64-unknown-linux-musl --lib`; it could not complete because ring's native build needs the unavailable `x86_64-linux-musl-gcc`. Linux and macOS execution is unavailable here and is not claimed verified. Windows console behavior, inherited lifetime handles, native sharing conflicts, and failed-deletion retry were executed on Windows. Ordinary pipe-based test runs may lack a console; the separate hidden-console recipe asserts that the interruption test actually ran.

## Actual mock installation

Latest successful run: `target/acceptance/ccf04c75-6374-4742-84e2-0401de236f64/report.json`. The regenerable latest report is `target/acceptance/latest.json`.

| Evidence | Value |
| --- | --- |
| Root | `C:/Users/ZackaryWang/Documents/GitHub/saucepan-2/mock` |
| App | `f0ff433d-18d8-4265-b32c-90969a496d18` |
| Current self-management claim | `727d1505-2f60-4052-aa81-90f8d8122f32` |
| Build A | `saucepan 0.1.0+acceptance-a` |
| A SHA-256 | `43b584452e19cba8775a4efe0d9d99ff7444e66bc339e95691489194bb06499a` |
| A output (now reclaimed) | `c67eb9f9-7bf8-4978-b4cd-c67e3320ee34` |
| Build B | `saucepan 0.1.0+acceptance-b` |
| B SHA-256 | `07c9c5e42cb61cb450f53a2f3929231a0a3b1585a45693e3c307b87dfb90cace` |
| Active B output | `503b5a44-0618-46c0-a684-eb3aca76daad` |
| Active executable | `saucepan.exe` |
| Ordinary reclaimed output | `c87dc1bf-c792-4fc7-aada-189d97f4c8a6` |
| Reacquired output | `2aaa3ce4-42bd-4508-90a2-551543b54aac` |

The executable digests were computed from the actual release builds and compared with acquired store bytes. The old process reported `acceptance-a` and the same mock root after B activated. A deliberately invalid executable failed with Windows error 216 and left B active. The release source has no repository working-data directory. All harness commands passed an explicit mock root; the child root was checked using filesystem identity. No production store was selected or initialized. Previous fixture runs and their provenance remain inspectable.

## Scenario evidence

| Spec scenarios | Executed evidence |
| --- | --- |
| App/output records coexist; natural structure; package-owned index.json | contracts, local, remote ZIP tests; mock source/index/store records |
| Interrupted staging/publication/claim update; complete old or new binding | core/index recovery tests before and after rename, plus activation journal replay |
| Legacy root preservation; explicit root across shim/child | contracts and CLI tests; acceptance child root identity and post-server shim commands |
| Latest/tag release assets without cloning | remote fixture tests; both actual self-managed release builds |
| Shared discovery across platforms; exact 24-hour expiry; backward clock; forced checks; failed timestamps/stale fallback | remote tests and controlled-clock acceptance commands |
| Ordered platform filters; fallback; zero/ambiguous matches | policy filter tests and shared-platform ZIP test |
| Tracked repository revisions; exact pins; immutable prior output; shared origin aliases | Git core/CLI two-revision fixture |
| Required Git dependencies and safe links | real submodule and LFS fixtures; corruption/missing/escaping link failure; logical link cycle/bounds tests |
| URL/local refresh behavior | URL test without validators; changed local files and versioned output tests |
| Distinct output provenance; compatible reuse; requested duplicate; differing policy | local, remote, concurrent acquisition, reclaim, and acceptance tests |
| Unsafe/incomplete contents never published | path/tree/ZIP fixtures, CRC/SHA mismatches, failed binary selection, bounded Git export, failed local selection |
| Native/CLI parity; unscoped acquisition | CLI/local/remote/Git tests, inspectable mock commands |
| Multiple selections per app; independent app updates; release/filter behavior | claims test and two ordinary acceptance apps |
| Reclaimed claims remain visible; explicit reacquisition | reclaim tests and eleven-entry acceptance history |
| Latest ten, custom count, maximum policy demand, reuse order | reclaim tests, managed keep-one tests, eleven-entry acceptance history |
| Pending deletion and retry without source removal | Windows held-file deletion test; source/provenance checks |
| Active protection, launch/reclaim coordination, target exit, launcher death | managed-process test with actual copied executable and inherited-handle check |
| Binding remains old until activation; failed probe/platform/path; rollback | managed tests and actual A/B/failed-candidate acceptance |
| Shim streams/status/interruption and no recursive target lookup | managed child tests, hidden-console Ctrl-Break test, shim direct binding implementation |
| Self-update while old target runs, distinct real builds, stable shim | release acceptance harness and retained mock installation |

## Review and boundaries

The supplied package ownership is retained: models define typed records; matching core modules own discovery, claims, policy, publication, and reclaim; activation/launch/bootstrap coordinate application behavior; utils contains no imports from core/models/cli. There is no graph runtime, inherited schema version, legacy decoder, or migration. U5 adapts portable logical-link mechanics from the sibling implementation without importing its domain models or authority protocol. Process execution uses process-wrap's native grouping, and Git remains the acquisition owner for repository/LFS data.

Lifetime markers use native open-handle/lock evidence instead of PID-only claims. Managed targets must preserve the inherited lifetime handle; deliberately closing inherited handles or escaping normal process lifetime is outside this contract. ZIP supports the enabled codecs, raw downloads are explicit, and publication does not promise universal power-loss filesystem durability. The fixture HTTP endpoint is temporary and explicit; offline launching uses installed content, while future discovery needs a live source.

The implementation files were reviewed for unfinished provider/shim stubs and utility/domain imports; none remain. Repository status contains the authorized new implementation, planning artifacts, tests, and documentation. Generated builds and mock contents are ignored. No Git commits or archive were created.

Proposed remaining checkpoint commits: `feat: add independent application claims`, `feat: reclaim outputs with native execution leases`, `feat: activate managed applications through stable shims`, and `test: demonstrate Saucepan release self-management in mock`.
