# Tasks

## 1. Marker-free non-process access

- [x] 1.1 Resolve metadata directly and create inherited markers only at the dedicated launch boundary while retaining shared entry coordination; document the distinction and verify repeated native/CLI lookups, full checks and native guards leave no markers, while existing lease, managed launch and run regressions pass. Review and commit this coherent checkpoint before task 2.

## 2. Cleanup independent of output retention

- [x] 2.1 Sweep closed recognized markers during reclaim using nonblocking entry coordination and native ownership checks; document cleanup timing and verify retained/active output cleanup, busy-entry creation windows, live inherited handles, process exit/failure, and unrelated-file preservation.
- [x] 2.2 Run formatting, strict Clippy, all-target tests, Windows console checks, isolated fresh/previous-launcher acceptance and strict OpenSpec validation; record concrete marker counts and platform limitations, review the diff and commit the verified cleanup checkpoint without OpenSpec change artifacts.
