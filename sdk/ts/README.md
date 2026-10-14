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
