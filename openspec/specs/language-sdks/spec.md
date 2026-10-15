# language-sdks Specification

## Purpose

Let Python and TypeScript applications use Saucepan's current resource contract through typed calls while preserving explicit identity, policy, failure and content-lifetime semantics.

## Requirements

### Requirement: Client construction has no operational side effects
Both SDKs SHALL accept an explicit executable and optional store root. With no executable override they SHALL invoke Saucepan through PATH when an operation is requested; with no root override they SHALL delegate store selection to Rust. Importing, constructing or deriving an application context SHALL NOT spawn processes, initialize stores, download/build binaries, register resources or contact a keyring. Fully portable root-free requests SHALL remain root-free.

#### Scenario: Construct an unused client
- **WHEN** a caller constructs a client with no Saucepan executable installed
- **THEN** construction succeeds without effects and an attempted operation reports the missing executable distinctly

#### Scenario: Root-free composition
- **WHEN** a caller uses a supported inline no-store operation without a home or explicit root
- **THEN** the SDK does not discover or create a store and delegates the supplied portable request unchanged

### Requirement: Typed operations preserve the current resource contract
Both SDKs SHALL provide equivalent typed operations for explicit initialization/application configuration, source registration/discovery/visibility, record inspection, immutable policy registration/lookup/adoption, scoped claim acquisition/query/context/update/recovery/release, retrieval and acquisition, exact paths and verification, reclaim/purge, registered group operations, pins and guarded execution. They SHALL additionally expose Rust-owned profile listing/expansion, effective-policy explanation and explicit managed install, bootstrap, update-app, self-update, rollback, uninstall, operations inspection and reconciliation. Requests SHALL accept objects without caller-authored JSON files. Policy UUID references, explicit portable catalogs, omitted optional values, exact output identities and provenance SHALL retain Rust CLI meaning. SDKs SHALL NOT evaluate policies, infer source aliases or latest outputs, reconstruct store paths, decode earlier schemas or provide legacy method adapters. Standalone shim install/replace/adopt and executable provisioning SHALL remain excluded and documented as such.

#### Scenario: Query a registered policy
- **WHEN** either language looks up a known policy UUID
- **THEN** it returns that exact immutable definition using the current policy command without adopting or modifying it

#### Scenario: Optional policy omitted
- **WHEN** a caller omits acquisition verification, identity or pin options
- **THEN** the SDK does not insert policy objects, create an app or pin, or replace omitted fields with null/default values

#### Scenario: Expand before explicit installation
- **WHEN** a caller requests an installation profile through either SDK
- **THEN** Rust returns equivalent flat definitions and refs without registration, fetching or installation
- **AND** the caller separately registers required definitions and supplies the installation request, built shim and explicit operation inputs

### Requirement: Identity and scope boundaries are explicit
SDKs SHALL support trusted local operations and explicit app-bound operations. Credentials SHALL be separately optional caller-owned file paths. Scope SHALL never be invented from application names, working directories or group keys. Conflicting bound and per-operation app identities SHALL fail before execution. Current Rust source/scope/action rules SHALL remain authoritative; app denial SHALL NOT trigger unrestricted fallback. SDKs SHALL NOT log or embed credential bytes in argv, exceptions or payload scratch.

#### Scenario: Credential-free scoped application
- **WHEN** an app without credential requirements queries or acquires outside its configured scope
- **THEN** the SDK preserves the Rust denial and does not retry as a local caller

#### Scenario: Conflicting app target
- **WHEN** an app-bound client receives a claim request naming a different application
- **THEN** it rejects that context conflict without invoking the executable

### Requirement: JSON operations distinguish domain and transport failures
JSON operations SHALL use shell-free literal argv, UTF-8 and the documented CLI error envelope. SDK failures SHALL distinguish Saucepan's code/message/details from missing executable, process launch failure, timeout, capture limit and malformed protocol. They SHALL NOT interpret English messages or guess domain errors from process status alone. Inputs and captured streams SHALL be bounded, temporary payloads SHALL be isolated per call, and owned scratch SHALL be cleaned on success and failure. Response decoding SHALL reject malformed required shapes and nonrepresentable portable integers rather than silently rounding. No failure SHALL return partial success data.

#### Scenario: Path and payload contain special characters
- **WHEN** executable paths, request values or arguments contain spaces, Unicode or shell metacharacters
- **THEN** values reach the intended process literally without shell interpretation and temporary files are cleaned afterward

#### Scenario: Output is malformed or excessive
- **WHEN** a JSON command returns invalid UTF-8, invalid JSON, an unexpected required shape or excessive capture
- **THEN** the SDK reports a distinct protocol/transport failure without fabricating a Saucepan result

#### Scenario: Domain failure
- **WHEN** Rust rejects a stale expected revision with its JSON error envelope
- **THEN** callers receive the same revision_conflict code and structured details in either language

### Requirement: Uncertain mutation outcomes are not retried automatically
SDKs SHALL preserve caller-supplied keys and expected revisions and SHALL NOT automatically retry mutating operations or compensate by deleting resources/releasing pins. Transport failure after possible execution SHALL identify that a mutation outcome can be indeterminate. Documentation SHALL explain reconciliation through record inspection and supported same-key receipt retry, without claiming that every command is idempotent.

#### Scenario: Prepare-and-pin response is lost
- **WHEN** the command might have committed before its response is lost
- **THEN** the SDK reports uncertainty without changing the key, unpinning or repeating preparation, and the caller can explicitly reconcile with the original receipt request

### Requirement: Paths and durable retention are distinct
Bare source/output inspection and ordinary group preparation SHALL return addresses/records without representing them as live Python/JavaScript leases. Pins SHALL require explicit caller requests and explicit release. Inspection SHALL preserve exact targets, owner, policy evidence, state and current availability supplied by Rust. Scope exit, garbage collection, client disposal and updater exit SHALL NOT release durable pins. Temporary results that require a retained native lifetime SHALL only be exposed through supported guarded execution or caller-owned delivery, not a false in-process handle.

#### Scenario: Another process reads a pinned old output
- **WHEN** an SDK caller prepares/pins A, exits, and another caller selects B
- **THEN** a consumer can still read pinned A until explicit release, independently of the SDK process lifetime

#### Scenario: Existing receipt is returned on retry
- **WHEN** a pin request returns an existing active receipt
- **THEN** disposing of the returned language object does not release that existing retention obligation

### Requirement: Guarded execution preserves raw process outcomes
SDKs SHALL expose the supported output/claim/group, retrieved-material and acquired-material run operations through Rust's launch boundary. Child argv SHALL follow `--` literally, streams SHALL be inherited by default, and observed exit status/signals SHALL be returned without parsing child output as JSON API success or domain failure. Guarded execution SHALL not have an implicit execution timeout. Explicit cancellation SHALL not claim all descendants have stopped or release unrelated durable retention. Rust-owned descriptors and lifetime markers SHALL not be deleted by SDK scratch cleanup.

#### Scenario: Child emits an error-shaped JSON document
- **WHEN** a launched consumer writes JSON resembling a Saucepan error and exits nonzero
- **THEN** the SDK returns the raw run outcome and does not reinterpret it as a Saucepan domain exception

#### Scenario: Consumer survives its wrapper
- **WHEN** the wrapper exits while a supported consumer retains its inherited handles
- **THEN** SDK cleanup does not revoke protection or delete the native group descriptor, and reclaim preserves content until the protected lifetime ends

### Requirement: Runtime transport boundaries are explicit
Python SHALL support Python 3.12 or newer and the default TypeScript transport SHALL support Node 22 or newer. TypeScript callers SHALL be able to import the typed client with an explicitly supplied transport without eagerly requiring Node process/filesystem modules. SDKs SHALL NOT pretend to provide subprocess access in a browser-only runtime or implicitly discover application-specific bridges. Consumer examples SHALL identify required host capabilities.

#### Scenario: Renderer supplies a host transport
- **WHEN** a TypeScript caller imports the transport-independent client and supplies a compatible host transport
- **THEN** operation mapping and typed results remain reusable without the SDK detecting browser globals or installing a bridge

### Requirement: SDK lifecycle results distinguish completion from pending work
Lifecycle SDK operations SHALL decode documented receipt responses on exit 0 and exit 2, preserving typed operation state and the observed command exit code. Complete receipt/exit combinations SHALL be validated separately from pending, failed and conflict results; operations inspection SHALL retain its read-only command semantics. Reconciliation lists SHALL preserve each operation's status. Domain error envelopes SHALL remain Saucepan errors and malformed or inconsistent receipts SHALL be protocol errors, with mutation uncertainty when execution might have occurred. Other operations SHALL retain their current exit handling. The SDK SHALL NOT automatically retry, reconcile, activate, release retention or compensate for a non-complete result.

#### Scenario: Windows launcher cleanup remains pending
- **WHEN** uninstall returns a valid cleanup-pending receipt with exit 2
- **THEN** both SDKs return the pending operation and exit code without claiming removal completed or treating valid receipt JSON as a malformed domain error

#### Scenario: Malformed lifecycle response
- **WHEN** a lifecycle command might have run but emits an invalid or exit-inconsistent receipt
- **THEN** the SDK reports a potentially indeterminate protocol failure rather than retrying or fabricating completion

### Requirement: Profiles preserve SDK identity and content lifetime boundaries
SDK profile calls SHALL delegate recipe expansion to Rust without maintaining language-specific policy tables. Managed setup SHALL require an explicitly available manager and caller-provided built shim. Client construction and package installation SHALL remain side-effect-free. Profile use SHALL NOT invent app scope, register identity, enable credentials, fall back after denial or create/release pins. Temporary-profile results SHALL use existing guarded execution or caller-owned delivery; the SDK SHALL NOT return a temporary path as though its native lifetime survived command exit.

#### Scenario: Temporary command consumes material
- **WHEN** a caller uses a temporary profile with a guarded run operation
- **THEN** the child consumes material within its native lifetime and SDK disposal does not claim an independent retained handle

#### Scenario: Bound setup operation is administrative
- **WHEN** an app-bound client attempts a setup or registration command reserved for local administration
- **THEN** the operation is rejected without silently dropping the app context
