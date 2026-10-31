# Managed setup SDK calls

Use native manager 0.12.0 with SDK 0.4.0 (or 0.11.0 with 0.3.0) from the same
release. Manager v0.10.0 and SDK 0.2.0 do not expose this setup contract.
Installing or constructing an SDK client never downloads native binaries.

```python
from saucepan_sdk import Client
client = Client(executable=external_manager, root=selected_root)
observation = client.setup_status()
result = client.setup(built_shim, tag=release_tag, operation=operation_uuid)
# Inspect result['exit_code'] and result['result']; exit 2 is a durable receipt.
```

```typescript
const client = new Client({executable: externalManager, root: selectedRoot});
const observation = await client.setupStatus();
const result = await client.setup(builtShim, {tag: releaseTag, operation: operationUuid});
```

Explicit migration adds Python `migrate_unmanaged=True, expected_sha256=digest`,
or TypeScript `migrateUnmanaged: true, expectedSha256: digest`, and requires an
explicit operation UUID. The manager verifies and backs up the exact raw file;
the SDK does not infer consent or implement ownership logic. App-bound clients
reject setup/status without dropping their identity.

`self_update(app, claim, tag=release_tag, operation=operation_uuid)` and
`selfUpdate(app, claim, {tag: releaseTag, operation: operationUuid})` forward an
optional exact tag. Omitting it preserves existing selector behavior.

Setup and setup-status use separate schema-version-1 decoders. Ready/complete
results require installation identities; noncomplete setup receipts retain
operation and backup/retry data. Lost or malformed responses after possible
mutation remain indeterminate transport failures. Neither SDK retries mutations
or restores backups. Consult [native setup and recovery](../docs/managed-setup.md).
