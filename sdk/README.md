# Language SDKs

`python/` and `ts/` adapt the current Rust CLI. Rust owns policy evaluation, source retrieval, output publication and retention. SDK installation does not install the manager. No external consumer repository is migrated by this work.

The operation inventory under `contracts/` tracks argument layouts, types, identity and real-binary coverage. Managed executable installation, bootstrap, activation, shim maintenance and uninstall wrappers are excluded from the initial resource-consumer API.

Paths and retention are different: bare lookup is read-only, guarded execution retains resources through the native launch boundary, and durable pins last until explicit release. Keep consumer writes outside immutable managed outputs.
