"""Explicit client context. Resource methods are thin mappings to the Rust CLI."""
from dataclasses import replace

from .errors import TransportError
from .transport import CliTransport, Config, Transport


class Client:
    def __init__(self, *, executable: str = "saucepan", root: str | None = None,
                 timeout: float | None = 120, capture_limit: int = 16 * 1024 * 1024,
                 input_limit: int = 16 * 1024 * 1024, transport: Transport | None = None):
        self._config = Config(executable=executable, root=root, timeout=timeout,
                              capture_limit=capture_limit, input_limit=input_limit)
        self._transport = transport if transport is not None else CliTransport()

    def for_app(self, app: str, *, credential_file: str | None = None) -> "Client":
        client = Client(transport=self._transport)
        client._config = replace(self._config, app=app, credential_file=credential_file)
        return client

    def _identity(self, app: str) -> str:
        if self._config.app is not None and app != self._config.app:
            raise TransportError("context_mismatch")
        return app
