# Tasks

Each numbered group is a dependency-ordered checkpoint. Complete its verification and documentation before proceeding to dependent work. Inspect the diff and author a focused zmem commit using explicit paths, excluding active OpenSpec change files and unrelated work. Do not modify external consumers, historical mock data, or the pending resource-group change; do not publish packages or dispatch hosted workflows.

## 1. Shared contract and Python transport

- [x] 1.1 Create the SDK layout and shared operation inventory from the current CLI and Rust models. Map every operation listed in the design to argv, input/output types, identity requirements, JSON/run mode and a real-binary test reference; verify the inventory against current CLI help and document excluded executable lifecycle operations.
- [x] 1.2 Implement Python process and payload-workspace utilities using standard library primitives. Verify literal Unicode/metacharacter arguments, simultaneous pipe draining, capture bounds, deadlines, missing executables, concurrent payloads, size limits and cleanup with real child processes and temporary files.
- [x] 1.3 Implement the inert Python client configuration and JSON transport, including error-format selection, omitted fields, credential-file references, portable integer bounds and response-shape checks. Test domain versus transport errors, malformed responses, mutation uncertainty and absence of implicit retries, initialization or version probes.
- [x] 1.4 Establish Python 3.12 packaging, typing and transport documentation; run the focused unittest suite and mypy, inspect the diff and checkpoint the verified shared-contract/Python-transport slice.

## 2. Python resource and lifetime APIs

- [x] 2.1 Add typed Python setup, source, policy, query and claim operations from the inventory. Test anonymous lookup, explicit app/scope propagation, context mismatch rejection, policy lookup/adoption and denied access without identity fallback against an isolated real binary; document minimal anonymous and scoped examples.
- [x] 2.2 Add retrieve, acquire-retrieved, source-path, path, verify, reclaim and purge-retrieval operations. Verify cached and root-free modes, Git source versus copied-result paths, update/context/recovery behavior and preservation of caller-selected policies using local Git or loopback fixtures.
- [x] 2.3 Add registered-group and durable-pin APIs with explicit owner identity and release. Verify registration/replacement/removal, revision conflicts, preparation failure atomicity, prepare-and-pin retry behavior and pin protection across reclaim; document that returned paths alone do not retain outputs.
- [x] 2.4 Add all designed run methods, delegating lifetime guards to native commands. Test raw exit/signal/streams, JSON-looking child errors, literal arguments, inherited and bounded captured output, no default timeout and explicit cancellation without auto-unpin; document supported child-chain limits.
- [x] 2.5 Run Python inventory coverage, typing and real-binary suites; build and inspect a wheel/sdist and import an installed package outside the checkout without Rust on PATH. Finish Python examples and checkpoint the verified resource/lifetime API slice.

## 3. TypeScript transport and runtime boundary

- [x] 3.1 Establish the Node 22 ESM package, TypeScript declarations, lockfile and transport-independent client export. Verify importing that export does not load Node process/filesystem modules, client construction causes no I/O, and an explicitly injected transport can execute a contract fixture without a host bridge implementation.
- [x] 3.2 Implement Node process/payload utilities and the default Node transport using built-ins. Verify both-stream draining, literal argv, UTF-8, limits, temporary-file closure/cleanup, deadlines and cancellation with real child processes, including a Windows-compatible fixture layout.
- [x] 3.3 Add typed JSON decoding/error handling and shared portable integer checks before lossy conversion, preserving omitted fields and caller identity. Test the shared failure fixtures and indeterminate mutation outcomes without fallback, retry or compensating actions.
- [x] 3.4 Run focused node:test and TypeScript checks, document default versus injected transport entry points and runtime prerequisites, and checkpoint the verified TypeScript transport slice.

## 4. TypeScript resource and lifetime APIs

- [x] 4.1 Implement setup/source/policy/query/claim and composition APIs against the shared inventory. Verify anonymous/scoped behavior, explicit policy adoption, cached/root-free results and claim update/recovery with real-binary fixtures matching the Python contract.
- [x] 4.2 Implement registered-group, pin and all designed run APIs. Verify revision conflicts, failed preparation, retry identity, reclaim protection, explicit unpin and raw child results; document that disposal, cancellation and returned paths do not release or create durable retention.
- [x] 4.3 Run inventory coverage, TypeScript and Node real-binary suites; build ESM/declarations and smoke-test an installed built local folder outside the checkout. Finish TypeScript examples and checkpoint the verified API slice.

## 5. Installed packages and consumer integration contracts

- [x] 5.1 Complete self-contained Python wheel/sdist metadata and source-install instructions. Test local, editable and commit-pinned Git-subdirectory installs using an isolated temporary Git fixture, plus wheel/sdist installs outside the checkout; verify README, license and py.typed contents and absence of automatic native installation.
- [x] 5.2 Complete TypeScript prepack/build and package-file allowlisting. Inspect and install the tarball outside the checkout, compile a consuming typed example and execute an installed API fixture without TypeScript or Rust build tooling in the consumer; document built-local-folder and tarball installation accurately.
- [x] 5.3 Add cross-language real-binary tests that create and inspect/use/release shared records under matching identities. Check every inventory operation has exercised coverage and compare stable fields/error codes with direct CLI results rather than duplicated implementation mocks.
- [x] 5.4 Add and document Power Eagle-shaped delayed-reader/pin handover, Zush-shaped command/hook/service lifetime, and PowerSpec-shaped read-only lookup/explicit upgrade/separate-scope fixtures. Verify their retention and refresh behavior in isolated roots and identify remaining consumer wiring without claiming external app migration.
- [x] 5.5 Run the installed-package, parity and consumer-fixture checks, update the root README and SDK guide with tested commands/runtime boundaries and dependency inclusion/exclusion reasons, and checkpoint the verified distribution/consumer contract slice.

## 6. Release artifacts and selective CI

- [x] 6.1 Extend the release helper with a validated SDK manifest containing package versions, exact filenames and tested Rust tag/commit. Test package/source metadata agreement, archive contents, the existing seven ZIPs plus exact SDK asset allowlist, checksums and rejection of missing/extra/duplicate/unsafe/inconsistent artifacts.
- [x] 6.2 Add reusable SDK checks with manual language/OS selection and release invocation. Cover Python and TypeScript process contracts on Windows/macOS/Linux with one native-manager build per job, without repeating the Rust all-target suite or architecture packaging for each SDK; retain manual/release-only triggers.
- [x] 6.3 Wire a single platform-independent SDK packaging job and installed-artifact verification into release publication dependencies and asset collection. Verify workflow references and release-helper tests locally; document versions, manifest, checksums and selective manual commands, with no registry publication or hosted dispatch.
- [x] 6.4 Review the focused release/workflow diff, run applicable local checks and checkpoint this verified slice, explicitly recording that unrun hosted platform jobs remain unverified.

## 7. Integrated acceptance

- [x] 7.1 Run the combined SDK type checks, tests, package builds, installed-artifact and cross-language acceptance checks against the current local binary, plus affected release-helper checks. Reconcile documentation and inventory with the results; record exact runtime/OS evidence and unavailable native-keyring or cross-platform verification without treating injected custody as native evidence.
- [x] 7.2 Validate this OpenSpec change strictly, review all requirement scenarios against implementation evidence, confirm no compatibility aliases/schema coercion or unrelated repository edits were introduced, and finish any coherent verified reconciliation checkpoint. Report remaining verification limitations and leave sync/archive for their requested workflow.
