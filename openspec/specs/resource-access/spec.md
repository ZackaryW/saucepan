# resource-access Specification

## Purpose

Provide efficient exact-output lookup and explicit resource lifetimes so applications and external processes can safely consume stored content during reclaim.

## Requirements

### Requirement: Bare paths are addresses without a retained lifetime
The existing `path ENTRY_UUID` command SHALL continue to return an absolute directory within the explicitly selected store. It SHALL resolve the exact output UUID and report the selected verification mode. It SHALL NOT select a newer entry, acquire content, refresh a source, create a claim, or retain protection after the operation ends. The native API and documentation SHALL distinguish bare lookup from leased access and explain that reclaim can invalidate an unleased returned path.

#### Scenario: Reading an old entry after a newer acquisition
- **WHEN** a caller resolves an older ready entry UUID after a newer version has been acquired
- **THEN** the result addresses that older entry and does not follow latest-release metadata or the app's active binding

### Requirement: Consumers can hold resource leases
The native API SHALL return the resolved output UUID, directory, and verification level together with a non-serializable lifetime guard. Obtaining this guard SHALL coordinate availability checking with reclaim so that no deletion can occur between successful selection and protection. While any live lease remains, policy reclaim SHALL defer otherwise eligible deletion and report the resource as in use. Releasing the final lease SHALL remove lease-based protection without changing retention order, provenance, claims, or other protections defined in [output-policies](../output-policies/spec.md). The guard SHALL protect against Saucepan reclaim, not external file edits by the local user.

#### Scenario: Multiple consumers use an otherwise evictable output
- **WHEN** two consumers lease an entry outside its retention window and one releases its lease
- **THEN** reclaim still defers deletion until the remaining consumer releases its lease
- **AND** a subsequent reclaim can remove its content without erasing provenance

#### Scenario: Reclaim wins the acquisition race
- **WHEN** reclaim marks an entry unavailable before a consumer acquires its lease
- **THEN** lease acquisition fails without returning a usable path or reviving the old entry

### Requirement: Claim-based access selects and protects one consistent output
A caller SHALL be able to acquire a resource lease using an app UUID and claim UUID in a single coordinated operation. Concurrent claim updates SHALL produce a lease on either the complete old selection or the complete new selection. The returned lease SHALL identify which output it protects and SHALL remain attached to that output after the claim changes or is released. Listing filters SHALL NOT implicitly create or grant a claim.

#### Scenario: A claim updates during access
- **WHEN** an application updates a claim while another consumer obtains a lease for that claim
- **THEN** the consumer receives one valid protected output with its matching directory
- **AND** any subsequent update leaves that lease's output identity unchanged

### Requirement: External processes have a leased resource workflow
The CLI SHALL support `run ENTRY_UUID [--verification metadata|full] -- PROGRAM ARG...` with an explicit store root. It SHALL resolve and protect the entry before starting the program, set `SAUCEPAN_RESOURCE_DIR` to the resolved directory in the child's environment, preserve literal arguments and inherited standard streams, and propagate exit status and supported platform interruption. It SHALL NOT expand a shell command, change the working directory implicitly, or inject Saucepan arguments into arbitrary programs. A missing program or failed validation SHALL leave no live orphan lease.

#### Scenario: A tool consumes data while reclaim runs
- **WHEN** a caller starts a tool through `run` and the tool reads a file through `SAUCEPAN_RESOURCE_DIR`
- **THEN** reclaim defers the leased output until the tool's protected lifetime ends, even if an app releases its claim meanwhile
- **AND** the tool's stdout and exit status are forwarded without extra JSON mixed into stdout

### Requirement: Process lifetime survives unexpected wrapper exit
Leased process execution and managed shims SHALL share the same native lifetime contract. Wrapper exit SHALL NOT make content reclaimable while a target retaining the inherited lifetime handle is still executing. Cleanup eligibility SHALL depend on native lifetime evidence, not a PID-only record or an elapsed lease timeout. Targets that deliberately discard inherited lifetime handles or escape the documented process-lifetime contract SHALL be explicitly identified as unsupported. Native API guards SHALL be non-inheritable by default; process inheritance SHALL be confined to the dedicated execution boundary.

#### Scenario: Windows wrapper is terminated before its consumer
- **WHEN** the wrapper is killed while its consumer retains the inherited lifetime handle
- **THEN** reclaim reports deferred use until the consumer exits and the native handle is released

#### Scenario: Unix wrapper becomes the consumer
- **WHEN** leased execution replaces the wrapper process with the target
- **THEN** protection survives the executable transition and is released when the target's protected lifetime ends

### Requirement: Shared access preserves managed-app behavior
Managed launch SHALL use the same resource-access primitives without losing explicit active binding, executable validation, root forwarding, or protection of old running targets across activation. Resource access SHALL use the current Saucepan data contract defined in [store-layout](../store-layout/spec.md), without inheriting an earlier schema version or requiring compatibility with pre-reimplementation stores. This SHALL NOT establish a general compatibility guarantee between different schema revisions or executable builds. The metadata-default verification behavior SHALL be documented for native and CLI callers; already-installed shim binaries SHALL NOT be assumed to gain new behavior until separately rebuilt and installed.

#### Scenario: An app activates a replacement during a leased launch
- **WHEN** a running shim target is replaced by a newly activated output
- **THEN** future launches select the new binding while the old target retains its lease and content
