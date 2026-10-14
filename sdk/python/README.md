# Saucepan Python SDK

Python 3.12+. Install this directory with `pip install ./sdk/python` from the repository root, or `pip install -e ./sdk/python` for development. A separately installed Saucepan executable is required only when an operation runs. Import and client construction do not locate it, create a store or contact a keyring.

```python
from saucepan_sdk import Client
client = Client()  # Saucepan on PATH; Rust selects the default root.
scoped = client.for_app("your-registered-app-uuid")  # no invented claim scope
```

`executable` and `root` accept explicit strings. Credentials are optional caller-owned file paths on `for_app`; credentials are never copied into SDK scratch. Python methods are synchronous. No legacy SDK aliases or native language bindings are provided.

The transport uses literal argv and per-call closed JSON files. JSON calls default to 120 seconds, 16 MiB per captured stream and 16 MiB per input (group/pin inputs are additionally capped at 1 MiB). Configurable limits must be positive. Integers must fit the shared JavaScript safe range. A transport failure exposes `kind` and `indeterminate`; a mutation may already have committed. Inspect the original record/key before explicitly retrying. No automatic retries or compensating releases occur.

`SaucepanError` exposes Rust's `code`, `message`, and `details`; exception rendering omits remote diagnostic text. `TransportError` distinguishes protocol, missing executable, launch, timeout and bounds failures. An injected `Transport` owns its process implementation; it must preserve these same contracts.

Guarded runs inherit streams and have no implicit timeout. Captured run streams are bytes, never parsed as JSON. Explicit cancellation kills/reaps the SDK-owned wrapper, not necessarily its descendants. Durable pins require explicit unpin; returned paths alone provide no retention.

Development: `uv run --group dev python -m unittest discover -s tests` and `uv run --group dev mypy` from this directory. Standard library process/files/typing cover runtime needs; Hatchling builds packages, mypy checks types and unittest exercises behavior. Pydantic, Execa-equivalents and async wrappers are deliberately absent because Rust owns semantic validation.
