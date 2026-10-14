"""CLI interpretation; utilities do not know Saucepan flags or envelopes."""
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, replace
import threading
from typing import Protocol

from .errors import SaucepanError, TransportError
from .utils.payloads import decode, payload_files
from .utils.process import RunResult, invoke


@dataclass(frozen=True)
class Config:
    executable: str = "saucepan"
    root: str | None = None
    app: str | None = None
    credential_file: str | None = None
    timeout: float | None = 120
    capture_limit: int = 16 * 1024 * 1024
    input_limit: int = 16 * 1024 * 1024


@dataclass(frozen=True)
class Payload:
    value: object


@dataclass(frozen=True)
class Call:
    argv: Sequence[str | Payload]
    mutates: bool = False
    shape: str = "object"
    required: tuple[str, ...] = ()
    payload_limit: int | None = None
    policies: object | None = None


@dataclass(frozen=True)
class RunOptions:
    capture: bool = False
    timeout: float | None = None
    cancel: threading.Event | None = None


class Transport(Protocol):
    def json(self, config: Config, call: Call) -> object: ...
    def run(self, config: Config, call: Call, options: RunOptions) -> RunResult: ...


def check_shape(value: object, call: Call) -> None:
    valid = (isinstance(value, list) if call.shape == "array" else isinstance(value, dict))
    if not valid or (isinstance(value, dict) and any(key not in value for key in call.required)):
        raise TransportError("protocol", started=True)


class CliTransport:
    def _invoke(self, config: Config, call: Call, options: RunOptions) -> RunResult:
        prefix = ["--error-format", "json"]
        for flag, value in (("--root", config.root), ("--as-app", config.app),
                            ("--credential-file", config.credential_file)):
            if value is not None:
                prefix.extend([flag, value])
        values = {str(i): arg.value for i, arg in enumerate(call.argv) if isinstance(arg, Payload)}
        if call.policies is not None:
            values["policies"] = call.policies
        limit = min(config.input_limit, call.payload_limit or config.input_limit)
        if limit <= 0:
            raise TransportError("invalid_input")
        try:
            with payload_files(values, limit=limit) as paths:
                if call.policies is not None:
                    prefix.extend(["--policies", paths["policies"]])
                argv = [paths[str(i)] if isinstance(arg, Payload) else arg for i, arg in enumerate(call.argv)]
                return invoke(config.executable, [*prefix, *argv], timeout=options.timeout,
                              capture=options.capture, capture_limit=config.capture_limit, cancel=options.cancel)
        except TransportError as error:
            error.indeterminate = call.mutates and error.started
            raise
        except OSError:
            raise TransportError("payload_io") from None

    def json(self, config: Config, call: Call) -> object:
        result = self._invoke(config, call, RunOptions(capture=True, timeout=config.timeout))
        try:
            if result.exit_code != 0:
                envelope = decode(result.stderr or b"")
                if isinstance(envelope, dict) and isinstance(envelope.get("error"), dict):
                    error = envelope["error"]
                    if (isinstance(error.get("code"), str) and isinstance(error.get("message"), str)
                        and isinstance(error.get("details"), dict)
                        and all(isinstance(k, str) and isinstance(v, str) for k, v in error["details"].items())):
                        raise SaucepanError(error["code"], error["message"], error["details"])
                raise TransportError("protocol", started=True)
            value = decode(result.stdout or b"")
            check_shape(value, call)
            return value
        except TransportError as error:
            error.indeterminate = call.mutates
            raise

    def run(self, config: Config, call: Call, options: RunOptions) -> RunResult:
        return self._invoke(config, call, options)
