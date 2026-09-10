# Safe Git symlink export verification

Recorded on 2026-09-10. No legacy branch source was used.

## Red-first evidence

- Pure resolver: two positive tests failed against the initial stub; all four path/cycle tests passed after implementation.
- Bounds: two new budget tests failed before entry/byte accounting; all six helper tests passed afterward.
- Git integration: all three initial safe-link tests failed with the old exporter, including cross-submodule and LFS cases; they passed after integration. Additional tests cover failed publication, mandatory checks with app policies disabled, unchanged reuse, rolling history, and exact pins.

## Executed checks

- Windows: `cargo test --locked` passed 80 unit tests, 10 integration tests, and one doctest. One native credential test was intentionally excluded. The refined historical-read regression also passed separately.
- Linux, Rust 1.97.1/Debian bookworm with Git LFS: 79 unit tests, 10 integration tests, and one doctest passed. One native credential test was excluded. Platform-specific filesystem tests explain the count difference.
- Formatting and Clippy with warnings denied passed. The first Linux container lacked Clippy; it was installed in a second isolated container and the strict check passed.
- TypeScript: five integration cases plus declaration checks passed on Windows Node 20 and Linux Node 22, including committed file/directory aliases through the real CLI.
- Shell: both local and Git fixtures passed in Windows Git Bash and Linux dash. The Git fixture commits a mode-120000 entry without creating a native symlink.
- Existing `.github/workflows/core-platforms.yml` runs the Rust tests on macOS, Windows, and Linux; `.github/workflows/sdks.yml` runs adapter tests on the same matrix. macOS was configured, not executed here.

## Previous-binary compatibility

A deterministic local repository and store were first acquired with the preceding release binary (SHA-256 `3e86b5ed35fb0b11b0f0b457bc1abceda3391de934d585a84ae036d6a8c60303`). Reacquisition with the new binary preserved the complete artifact record and retained ZIP bytes, and the original caller token verified the current view. No index migration or registration refresh was needed.

## Remote acceptance

The rebuilt Windows release acquired `https://github.com/github/gitignore.git` over HTTPS at `9e86bc12f67365b8dd974d3b3f09d166265c5530`. Full-tree and `Global` acquisitions shared the same source/snapshot. All three previously rejected links matched committed target bytes as ordinary files; mirrors, recorded origin, and app-view verification succeeded. Inspection of the retained ZIP verified those bytes, absence of symlink modes, and absence of `.git` entries.

Detailed local reports are under ignored `.codex/symlink-remote-report.json` and `.codex/symlink-compatibility-report.json`. They contain local artifact locations and are not required by tests or fresh clones. Public HTTPS acceptance does not establish private-credential or SSH coverage.
