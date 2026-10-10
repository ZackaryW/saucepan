# Spec Delta

## Purpose

Give Rust library and command-line consumers stable failure identities without requiring them to parse human-readable diagnostic messages.

## ADDED Requirements

### Requirement: Consumer failures have stable codes
Public failures SHALL expose a stable code, human-readable message, and bounded structured details. The contract SHALL distinguish `invalid_request`, `authentication_required`, `authentication_failed`, `key_unavailable`, `integrity_error`, `unknown_application`, `unknown_claim`, `unknown_output`, `content_unavailable`, `exact_content_unavailable`, `revision_conflict`, `claim_context_conflict`, `claim_overlap`, `policy_conflict`, `access_denied`, `source_error`, `io_error`, and `unsupported_store`. Equivalent Rust library and CLI failures SHALL have the same code; clients SHALL NOT need English substring matching. Error details SHALL redact credentials and protected metadata for unauthorized callers. Authentication SHALL precede disclosures about a protected application's private claims.

#### Scenario: A consumer handles reclaimed content
- **WHEN** an authorized consumer attempts to lease an unavailable claimed output
- **THEN** it can branch on `content_unavailable` independently of the message wording

#### Scenario: Unauthorized claim identity probe
- **WHEN** a caller without required proof supplies a claim UUID to a protected application
- **THEN** the response reports an authentication failure without revealing whether that private claim exists

### Requirement: CLI JSON errors preserve existing output and process contracts
The CLI SHALL accept `--error-format json` and emit command/preflight failures on stderr as one JSON object shaped `{ "error": { "code": "...", "message": "...", "details": {} } }`, with nonzero exit status and no success result. Argument errors SHALL support this mode when the format option is validly supplied; help and version SHALL remain successful informational operations. Normal successful stdout schemas SHALL remain unchanged. Recorded lifecycle outcomes SHALL retain exit 0 for complete and 2 for incomplete/failed/conflicted results, with structured reason codes. Executed child stdout, stderr, and exit status SHALL remain forwarded without wrapping a child's own errors as Saucepan errors.

#### Scenario: Invalid query in JSON error mode
- **WHEN** a query contains an unknown operator
- **THEN** stderr contains one `invalid_request` error object and stdout has no successful query result

#### Scenario: Child exits with its own error
- **WHEN** leased execution starts successfully and the child reports failure
- **THEN** its streams and status are forwarded without an added Saucepan JSON error envelope

### Requirement: Failed resource access releases partial native ownership
The Rust library SHALL return typed errors exposing code, message, and details on failed resource access. It SHALL release any partially acquired lifetime resources and SHALL NOT return a usable resource handle on failure. Error formatting SHALL preserve secret redaction.

#### Scenario: Failed lease acquisition
- **WHEN** native claim lease acquisition fails authentication or availability checks
- **THEN** a typed error is returned and no orphan lease or usable resource handle is returned
