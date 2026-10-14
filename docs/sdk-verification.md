# SDK verification — 2026-10-14

The Python and TypeScript adapters were exercised locally on Windows against
the current debug Rust manager. This report distinguishes package/fixture
evidence from hosted runs and actual consumer application migration.

Verified on Windows with Python 3.12.13/3.14.6 and Node 22.23.3/26.10.0:

- Python: 14 tests across real process transport, identity checks, all 46 resource
  operations, local Git, root-free composition and native lifetime/reclaim.
- TypeScript: nine Node tests plus four shared real-binary resource scenarios;
  all 46 operations covered. A module-loader test rejects eager Node process or
  filesystem imports when importing the transport-independent client.
- Three cross-language consumer fixtures: Python prepares/pins A, Node inspects
  it and prepares B, delayed direct reads still see A until explicit unpin;
  Zush-shaped command/hook/service resources remain available through cleanup;
  PowerSpec-shaped read-only lookup preserves selection and explicit preparation
  advances only the intended project scope.
- Wheel, source distribution and npm tarball installation outside the checkout,
  including operations against an explicit binary and imports with empty PATH.
  Local/editable and commit-pinned Git-subdirectory Python installation passed
  using an isolated temporary Git repository. TypeScript built-folder installation
  and a consuming declaration check (including rejected invalid inputs) passed.
- Python strict mypy and TypeScript strict compilation passed. Runtime
  dependencies remain standard-library/Node built-ins; no Cargo changes.

The resource-group Rust implementation was already separately verified and is
now synchronized and archived. This SDK change does not reimplement or change
that policy/storage engine.

- Twelve release-helper tests pass, including exact asset sets, package/source
  agreement, unsafe/duplicate members, checksums and mixed-commit rejection.
  A manifest was also generated from the actual built wheel, sdist and npm
  archive. All three affected workflows pass actionlint 1.7.12 with optional
  ShellCheck and Pyflakes disabled.

No hosted workflows, package-registry publication,
release/tag push or external consumer migration has been performed. Native
keyring custody is not established by these basic-store fixtures. Linux/macOS
process behavior and supported inherited-handle chains require their own runner
evidence; detached services and arbitrary handle-closing descendants still need
consumer lifetime wiring or explicit pins.
