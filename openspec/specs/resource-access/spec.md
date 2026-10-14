# resource-access Specification

## Purpose

Provide efficient exact-output lookup and explicit resource lifetimes so applications and external processes can safely consume stored content during reclaim.

## Requirements

### Requirement: Bare paths are addresses without a retained lifetime
The `path ENTRY_UUID` command SHALL return an absolute directory within the store selected by the default or overridden root in [store-layout](../store-layout/spec.md). It SHALL resolve the exact output UUID and report the selected verification mode. It SHALL NOT select a newer entry, acquire content, refresh a source, create a claim, or retain protection after the operation ends. The native API and documentation SHALL distinguish bare lookup from leased access and explain that reclaim can invalidate an unleased returned path.

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
A caller SHALL be able to acquire a resource lease using an app UUID and claim UUID in a single coordinated operation, after enforcing the application's configured credential requirement and source/claim action policy. App-scoped exact-output access SHALL enforce the same policy and SHALL NOT fall back to unrestricted store administration. An optional relative view SHALL be validated against the selected output manifest and containment boundary before exposing its directory. A claim lease SHALL protect the whole underlying output and SHALL NOT create another claim or automatically recover unavailable content. Concurrent claim updates SHALL produce a lease on either the complete old selection or the complete new selection. The returned lease SHALL identify which output it protects and SHALL remain attached to that output after the claim changes or is released. Listing filters SHALL NOT implicitly create or grant a claim.

#### Scenario: A claim updates during access
- **WHEN** an application updates a claim while another consumer obtains a lease for that claim
- **THEN** the consumer receives one valid protected output with its matching directory
- **AND** any subsequent update leaves that lease's output identity unchanged

#### Scenario: Native consumer retains a covered view during reclaim
- **WHEN** a Rust caller holds a lease for a contained claim view while another process releases the claim and attempts reclaim
- **THEN** the entire selected output remains protected until that guard and all other applicable protections end

#### Scenario: Native view guard is dropped
- **WHEN** the Rust caller drops the last guard protecting an otherwise reclaimable output
- **THEN** a later reclaim can delete its content without changing retained claim provenance

### Requirement: External processes have a leased resource workflow
The CLI SHALL support `run ENTRY_UUID [--verification metadata|full] -- PROGRAM ARG...` using the default or overridden root in [store-layout](../store-layout/spec.md). It SHALL resolve and protect the entry before starting the program, set `SAUCEPAN_RESOURCE_DIR` to the resolved directory in the child's environment, preserve literal arguments and inherited standard streams, and propagate exit status and supported platform interruption. It SHALL NOT expand a shell command, change the working directory implicitly, or inject Saucepan arguments into arbitrary programs. A missing program or failed validation SHALL leave no live orphan lease.

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

### Requirement: Source and acquired paths have distinct representations and lifetimes
Native and CLI access SHALL distinguish source material paths from acquired-content paths. Source descriptors SHALL report file, directory or Git repository representation and exact provenance; a bare repository SHALL not be represented as a checked-out directory. Native source access SHALL offer a non-serializable guard coordinating refresh and cleanup. Acquired access SHALL offer output lifetime protection or explicit caller ownership. Bare path lookups SHALL not extend either lifetime or perform updates. A native temporary acquired result SHALL retain any dependency guards needed for continued use.

#### Scenario: Inspect repository then use an acquired folder
- **WHEN** a caller accesses a retrieved Git repository and acquires its plugin subpath
- **THEN** source_path refers to repository data and the acquisition path refers to the copied plugin files
- **AND** each result identifies its ownership and whether a live guard is held

#### Scenario: CLI requests a temporary path without an owner
- **WHEN** a caller requests bare temporary retrieval output that would expire when the CLI exits
- **THEN** the CLI rejects it with guidance to use a child-consumer workflow or caller-owned destination

### Requirement: Retrieved-material execution retains source lifetime
The CLI SHALL provide run-retrieved FILE -- PROGRAM ARG... with literal argument forwarding, source-path and representation metadata in the child environment, inherited standard streams and child exit status. It SHALL retain cached source access or temporary material through the supported native child lifetime, including unexpected wrapper exit. It SHALL not expose a writable cache capability or silently substitute an acquired output. Failure to prepare or start the child SHALL leave no live orphan protection.

#### Scenario: Read original ZIP in a child
- **WHEN** a caller runs a consumer on a temporary ZIP retrieval
- **THEN** the child can read the original archive until its protected lifetime ends without an extracted output or persistent cache

### Requirement: Temporary acquired files have a process consumer workflow
The CLI SHALL provide run-acquired FILE -- PROGRAM ARG... composing retrieval and acquisition before starting a consumer. It SHALL expose acquired files through SAUCEPAN_RESOURCE_DIR and preserve literal arguments, standard streams and child exit status. Temporary copies and required parent guards SHALL survive the supported child lifetime, including unexpected wrapper exit. It SHALL reject a bare temporary acquisition result that would expire on CLI exit and SHALL clean up failed launches without orphan protection.

#### Scenario: Use a temporary Git subpath without persistent caching
- **WHEN** a caller runs an acquired consumer using temporary Git retrieval and temporary subpath acquisition
- **THEN** the child receives the copied subpath for its protected lifetime
- **AND** no managed retrieval, output or claim records are created

### Requirement: Group consumers receive named paths with shared lifetime protection
Native access SHALL expose the complete admitted group through one lifetime-bearing handle. The CLI SHALL provide `run-group FILE [--policies CATALOG] -- PROGRAM ARG...` using the existing default/explicit root and optional app/credential context. It SHALL admit the entire group before launching, forward literal arguments and inherited streams, preserve the working directory, and propagate child exit status and supported interruption behavior. It SHALL set SAUCEPAN_RESOURCE_GROUP_FILE to a bounded UTF-8 JSON descriptor whose members are keyed by request name and identify exact output UUIDs, optional claim UUIDs, selected directories, relative views and verification. The descriptor SHALL contain no secrets and SHALL NOT be accepted as a lease or authorization token. Group runs SHALL remove inherited singular resource/source variables from the child's environment without modifying the parent environment.

#### Scenario: A command uses hook and service repositories
- **WHEN** a child opens its group descriptor and reads command, hooks and services from three outputs
- **THEN** all three paths remain protected during the supported child lifetime without app claims being required for output-only groups
- **AND** no success JSON is mixed into the child's stdout

#### Scenario: Literal arguments and child errors survive forwarding
- **WHEN** a group consumer receives spaces or shell metacharacters in arguments and exits nonzero
- **THEN** arguments arrive unchanged and the CLI forwards the child's exit status without wrapping it as admission success

### Requirement: Group execution preserves every inherited resource lifetime
Dedicated group execution SHALL establish native ownership for every distinct output and the descriptor before starting the child. Supported children retaining inherited handles SHALL retain all resources and the descriptor after unexpected wrapper exit. Native in-process group guards SHALL remain non-inheritable by default. Preparation or launch failure SHALL leave no live orphan protection; closed markers and owned scratch SHALL be eligible for existing native-evidence-based cleanup. No PID-only or elapsed-time rule SHALL revoke protection. Ordinary native group access SHALL NOT create per-use process markers.

#### Scenario: Wrapper exits while several outputs are in use
- **WHEN** the wrapper terminates while a supported consumer retains the inherited handles and subsequently reads its descriptor
- **THEN** the descriptor and every group output remain available and reclaim defers their deletion until their protected lifetimes end

#### Scenario: A later inheritance step or spawn fails
- **WHEN** group setup fails after some native handles have been prepared
- **THEN** no child begins resource work and all attempt-owned handles are released so cleanup can remove closed markers and scratch

#### Scenario: Native group is unrelated to a spawned process
- **WHEN** a native caller holds a group and launches an unrelated child outside the dedicated execution boundary
- **THEN** that child does not inherit the group's protection and dropping the native group releases its guards
