# Saucepan TypeScript SDK

Typed, asynchronous Node.js client for the Saucepan **0.5.x central-store CLI**. Requires Node.js 20+ and a separately supplied executable. It has no runtime npm dependencies and does not download binaries. It is a Node SDK, not a browser SDK.

Build/package from this checkout:

```sh
cd sdk/typescript
npm ci
npm run build
npm pack
# In a consumer project:
npm install /path/to/saucepan-sdk-0.5.0.tgz
```

The package exports ESM JavaScript and TypeScript declarations. It has not been published by this change.

```ts
import { Saucepan } from 'saucepan-sdk';

const store = new Saucepan(); // ~/.saucepan/bin/saucepan[.exe]
// await store.init();        // first user-store initialization only
const token = await store.register('my-app'); // once per app
const app = store.forApp(token);
const acquired = await app.acquire({
  source: { provider: 'git', origin: 'https://github.com/your-org/assets.git', reference: 'main' },
  folder: 'themes',
});
console.log(acquired.directory);
const view = await app.view();
await app.verify(view);
```

Use your actual repository. Persist registration's token as JSON if needed, then construct `new Saucepan({ marker: '/path/to/.saucepanhash' })`. Marker files belong to the caller and are never rewritten. Passing `token` instead uses a private temporary JSON file for each call. `store.forApp('my-app')` selects ordinary mode; `store.forApp(token)` supplies proof. Registration/configuration settings remain in the encrypted central index.

Every method returns a Promise:

| Method | Result |
| --- | --- |
| `init()` | `{ created: true }` |
| `register(app, configuration?)` | Stable `AppToken` |
| `configure(configuration)` | `{ configured: true }` |
| `acquire(recipe)` | `Acquired`, including directory, revision and fallback/verification flags |
| `view()` | App's current `AppView` |
| `verify(view)` | `{ verified: true }` or rejection |
| `path(artifactId)` | Central directory or `null` |
| `mirror(artifactId, destination)` | `{ directory }`; occupied destinations fail |
| `history(sourceId)` | `SourceState` or `null` |
| `snapshot(sourceId, snapshotId, folder?)` | `Acquired` without advancing current |
| `sharedExecutable()` | User-level executable path reported by the CLI |

`Configuration` has optional `settings` and `filters` objects. Supply complete objects for each one you change:

```ts
await app.configure({
  settings: { retain_snapshots: true, verify_content: true, allow_local_fallback: false },
  filters: { source_ids: [], providers: ['git', 'local'] },
});
```

Omitted objects keep their centrally recorded values. Empty filter arrays match all. Recipes are a discriminated union for `git`, `url` (file or ZIP), and `local`; their JSON fields match the [central-store guide](../../docs/central-source-store.md). In a repository checkout that guide is at `docs/central-source-store.md`; it is not included in the npm package.

Options: `binary`, `app`, `marker` **or** `token`, `authoritative`, `timeoutMs` (default 0, unlimited), and `maxBuffer` (default 16 MiB per output stream). Pass an explicit executable path to avoid depending on placement. `sharedExecutablePath()` computes the default locally.

For isolated tests, supply `testStore: { root: '/short/test-root', key: '64 hex characters' }`. Both fields are required; there is no automatic test-mode fallback. Keep Windows test roots short, as the core currently has long-path limitations.

The client invokes the executable directly with argument arrays and no shell. Per-call request files are removed after success, failure, or timeout. Each call reads current CLI state; there is no SDK index cache, policy engine, permission administration, or token-refresh protocol.

With safe Git symlink export in the supplied CLI, relative committed links are returned as independent ordinary content. Targets resolve within the complete source tree before `folder` selection; no extra SDK option is needed. Unsafe, broken, cyclic, or excessive expansions fail even with repeated verification off. Local filesystem links and URL ZIP symlink entries remain unsupported.

`SaucepanError` preserves `exitCode` (null for launch/transport failures), `stdout`, `stderr`, `code`, `signal`, and `killed`. It does not apply the old Python SDK's exit-code categories or include the command/test key in its generated error message. JSON output errors use `INVALID_JSON`; unsupported versioned responses use `PROTOCOL_VERSION`. Nonzero exits reject; missing entries remain `null`. A timeout does not guarantee that the core made no changes before termination.

Validation, after building the Rust executable at the repository root:

```sh
npm test
```

Set `SAUCEPAN_TEST_BINARY` to test another compatible executable. Tests cover all methods, Git/URL/local input, quoting, stable markers, wrong proofs, concurrent request files, errors, and timeout cleanup. Type checks also verify invalid recipe/test-store shapes are rejected.
