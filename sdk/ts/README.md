# Saucepan TypeScript SDK

Requires Node 22+ for the default transport. From this folder run `npm ci` then `npm run build`; a consumer can install this built folder, or run `npm pack` and install the resulting `.tgz`. The tarball contains ESM JavaScript and declarations; consumers need neither TypeScript nor Cargo. Direct npm Git-subdirectory installation is not supported.

```ts
import { Client } from 'saucepan-sdk';
const client = new Client(); // No I/O; executable and root remain optional.
const bound = client.forApp('explicit-application-uuid');
```

`saucepan-sdk/client` is transport-independent at import time. `Client({ transport })` accepts an explicit host transport; the default Node transport is loaded only when an operation executes. There is no automatic browser bridge or `window.require` detection. Power Eagle must supply its own host integration where Node subprocess access is unavailable.

JSON calls default to 120,000 milliseconds and 16 MiB per stream/input (1 MiB group/pin input ceiling). Timeout and bounds are configurable. `TransportError.kind` distinguishes transport failures, and `indeterminate` warns that a mutation may have committed. `SaucepanError` preserves `code`, `remoteMessage`, and `details` without printing remote diagnostics in its exception text. No retries or compensating mutations are automatic.

Run options have no default timeout, inherit streams by default, and optionally capture byte arrays. `AbortSignal`/explicit deadlines reap the SDK-owned process without asserting descendants stopped. Child output is never decoded as domain JSON. Paths alone do not retain outputs and pins require explicit unpin.

Runtime dependencies are Node built-ins; TypeScript and @types/node are development dependencies and node:test supplies verification. Zod and Execa are excluded because bounded wire/process handling is sufficient and Rust remains the semantic authority. No binary installer or registry publishing hook is present.

## Resource workflow

```ts
const client = new Client({ executable: '/path/to/saucepan', root: '/disposable/store' });
await client.init();
const source = await client.registerSource({type: 'git', origin: '/path/to/local/repo'});
const app = await client.registerApplication({registration_key: 'my-tool', name: 'My tool'});
const bound = client.forApp(app.uuid);
const selected = await bound.ensureClaim(app.uuid, {
  source_uuid: source.uuid, selector: {type: 'ref', reference: 'HEAD'},
  scope: 'project-a', selection: {folder: 'skills'},
});
const views = await bound.queryClaims({app: app.uuid, scope: 'project-a', where: []});
// Read-only lookup preserves selection. Explicit update affects this claim.
const updated = await bound.updateClaim(app.uuid, selected.claim.uuid);
```

Method/options names are camelCase; Rust payload/result fields remain snake_case. Import request/response types from `saucepan-sdk/models/claim`, `/group`, `/pin`, `/source`, `/policy` or `/composition`. `querySources()` needs no app or JSON file and sees explicitly public sources. Unbound `acquire` provides trusted local acquisition; bound acquisition requires an explicit scope.

`retrieve` returns `{request, descriptor}` for original material, `sourcePath` performs read-only lookup and `acquireRetrieved` returns copied results. Supply explicit portable `policies` and caller-owned destinations for root-free no-store operations. Temporary results use `runRetrieved`/`runAcquired` to retain native lifetimes.

`registerGroup` stores intent and `prepareGroup` materializes it. To retain paths after the updater exits, explicitly pass `pin` and `expectedRevision` with a registered pin policy; inspect with `pin`/`pins` and release with `unpin`. Ordinary preparation and object disposal do not change pin ownership. Run options such as `{capture:true, signal}` accompany other options directly, and captured bytes are Uint8Array values.

Trusted administration may supply a caller-owned `credentialFile` to the constructor when configuring protected apps; this does not select app identity. App sessions use `forApp(uuid, {credentialFile})`. No credential bytes are read by this package.

Run `npm test` for transport and real-binary lifetime checks. Shared resource parity tests run from the repository root with `uv run --no-project python -m unittest discover -s sdk/contracts -p test_node_resources.py`. Set `SAUCEPAN_TEST_BINARY` explicitly or build the default debug binary first. Tests use isolated stores, local Git and the current wire contract, never real consumer projects.

## Profiles and managed applications

`await client.profile()` expands the manager's `managed` recipe without opening a root; `profile('explicit-update', {forInstall:true})` prepares ordinary installation policies. Explicitly `registerPolicies(profile.catalog)` and attach the refs. `explainPolicies(request, {policies:catalog})` reports choices/provenance without fetching. Types are exported from `saucepan-sdk/models/setup`; recipes and policy evaluation remain Rust-owned.

`installApplication(name, request, probe, builtShim, {scope, shim})`, `bootstrap(request, builtShim)`, `updateApplication`, `selfUpdate`, `rollback`, `uninstall` and `reconcile` return `{exit_code: 0 | 2, result}`. Exit 2 preserves a recorded noncomplete outcome, including failed/conflict status. `operations({app})` returns a read-only list. Inspect before explicitly reconciling or retrying the same `operation` UUID. No downloads, PATH edits, automatic retries, credentials or scopes are introduced. Standalone shim maintenance and activation remain CLI-only. See [the SDK guide](../README.md#profiles-and-managed-setup) and runnable `examples/profiles.mjs` / `examples/managed-setup.mjs`. Contract tests require the matching shim beside the test manager and use a loopback release fixture.
