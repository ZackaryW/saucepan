# Saucepan SDKs

All SDKs use an independently supplied executable; binary acquisition is maintained elsewhere.

| SDK | Target API | Status |
| --- | --- | --- |
| [TypeScript](typescript/README.md) | Central-store CLI 0.5.x | Typed asynchronous Node.js client |
| [Shell](shell/README.md) | Central-store CLI 0.5.x | POSIX functions, raw JSON output |
| [Python](python/README.md) | Legacy workspace CLI | `Workspace` runtime migration is still pending; current CLI fixtures do not establish runtime compatibility |

App registration/settings, scoped views, stable tokens, source acquisition, and retention remain core responsibilities. The TypeScript and shell SDKs do not read TOML settings or duplicate index policy.
