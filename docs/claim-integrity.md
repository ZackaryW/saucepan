# Integrity and custody

[← Saucepan guide](../README.md#integrity-and-access)

## Choose a store mode

New stores use basic checksums and need no OS keyring:

```powershell
saucepan init
```

For HMAC metadata verification backed by the native OS keyring, initialize a separate root:

```powershell
saucepan --root ./protected init --integrity keyring
```

Commands and shims use the stored mode automatically. Initialization rejects a retry with a different mode; there is no in-place conversion. An existing protected store fails if its key is unavailable, without replacing the key or downgrading protection.

Both modes keep metadata readable. Basic checksums detect inconsistency but cannot detect a filesystem writer who edits records and recomputes their checksums. HMAC detects changes made without the separate key; it does not encrypt metadata, prevent filesystem edits, or detect a complete replay of an earlier valid store without an external anchor. OS permissions remain responsible for file access.

## Optional application credentials

Credentials are independently opt-in in either store mode. An app session applies source, scope and action restrictions even when credentials are disabled. Public source discovery and public claim exposure are independent settings.

Use the [claim CLI](claim-cli.md#optional-credentials-and-administration) to enable, rotate or disable credentials. Pass proof through a caller-owned credential file. Rotation invalidates old proof without revoking existing native leases. In basic mode these controls enforce API behavior; they do not prevent a filesystem writer from replacing metadata and checksums.

## Metadata and recovery

Sources, outputs, applications, claims, policies, groups and pins participate in a verified catalog. Missing or altered records fail verification. Update records through the manager's commands rather than editing their JSON directly.

A verified journal lets reopening recover interrupted metadata publication. It does not provide a whole-filesystem power-loss durability guarantee. Published output contents remain immutable; use explicit payload verification when you need to check their bytes. See [resource access](resource-access.md).

## Native integration

`Store::create` uses basic integrity. `Store::create_with_integrity` selects a mode explicitly; `Store::integrity_mode()` reports the opened mode. Use `Store::session(app, optional_credential)` for application operations and the [native consumer guide](scoped-consumer.md) for ownership and lifetime examples.

Native custody uses Rust keyring packages. Its availability depends on the host's credential service. Basic-store tests and injected test keys do not demonstrate native custody; select native checks explicitly as described in [release checks](releases.md#checks-and-publication). Implementation rationale and dated results live with their OpenSpec changes.
