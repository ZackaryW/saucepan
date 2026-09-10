"""Synchronous adapter for the Saucepan central-store CLI."""
import copy
import json
import math
import os
import re
import tempfile
from pathlib import Path
from typing import Any, Mapping, Optional, Union

from ._runner import execute
from .errors import SaucepanError

Pathish = Union[str, os.PathLike]


def shared_executable_path() -> Path:
    return Path.home() / ".saucepan" / "bin" / ("saucepan.exe" if os.name == "nt" else "saucepan")


class Saucepan:
    """Use registered app contexts; all settings, filtering and caching stay in core."""

    def __init__(self, *, binary: Optional[Pathish] = None, app: Optional[str] = None,
                 marker: Optional[Pathish] = None, token: Optional[Mapping[str, Any]] = None,
                 authoritative: bool = False, test_root: Optional[Pathish] = None,
                 test_key: Optional[str] = None, timeout: Optional[float] = None):
        if marker is not None and token is not None:
            raise ValueError("Choose a marker file or a token, not both")
        if (test_root is None) != (test_key is None) or (test_root is not None and
                (not os.fspath(test_root) or not re.fullmatch(r"[0-9a-fA-F]{64}", test_key or ""))):
            raise ValueError("A test store requires a root and a 64-character hexadecimal key")
        if timeout is not None and (not math.isfinite(timeout) or timeout <= 0):
            raise ValueError("timeout must be finite and greater than zero, or None")
        self.binary = os.fspath(binary) if binary is not None else str(shared_executable_path())
        self._options = dict(binary=self.binary, app=app,
                             marker=os.fspath(marker) if marker is not None else None,
                             token=copy.deepcopy(token), authoritative=authoritative,
                             test_root=os.fspath(test_root) if test_root is not None else None,
                             test_key=test_key, timeout=timeout)

    def for_app(self, app: Union[str, Mapping[str, Any]]) -> "Saucepan":
        options = {k: v for k, v in self._options.items()
                   if k not in ("app", "token", "marker", "authoritative")}
        return Saucepan(**options, **({"app": app} if isinstance(app, str) else {"token": app}))

    def init(self) -> dict:
        return self._call("init")

    def register(self, app: str, *, settings: Optional[Mapping] = None,
                 filters: Optional[Mapping] = None) -> dict:
        return self._versioned(self._call("register", [app], settings=settings, filters=filters))

    def configure(self, *, settings: Optional[Mapping] = None, filters: Optional[Mapping] = None) -> dict:
        return self._call("configure", settings=settings, filters=filters)

    def acquire(self, recipe: Mapping[str, Any]) -> dict:
        return self._call("acquire", document=recipe)

    def view(self) -> dict:
        return self._versioned(self._call("view"))

    def verify(self, view: Mapping[str, Any]) -> dict:
        return self._call("verify", document=view)

    def path(self, artifact: str) -> Optional[str]:
        return self._call("path", [artifact])

    def mirror(self, artifact: str, destination: Pathish) -> dict:
        return self._call("mirror", [artifact, os.fspath(destination)])

    def history(self, source: str) -> Optional[dict]:
        return self._call("history", [source])

    def snapshot(self, source: str, snapshot: str, folder: Optional[str] = None) -> dict:
        return self._call("snapshot", [source, snapshot], folder=folder)

    def shared_executable(self) -> str:
        return self._call("shared-executable")

    @staticmethod
    def _versioned(value: Any) -> dict:
        if not isinstance(value, dict) or type(value.get("version")) is not int or value["version"] != 1:
            raise SaucepanError("Unsupported response format version", code="PROTOCOL_VERSION")
        return value

    def _call(self, command, positionals=(), *, settings=None, filters=None, document=None, folder=None):
        # Per-request ownership makes concurrent calls independent, including cleanup.
        with tempfile.TemporaryDirectory(prefix="saucepan-request-") as temporary:
            sequence = 0

            def json_file(value):
                nonlocal sequence
                path = Path(temporary) / f"{sequence}.json"
                sequence += 1
                descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
                    json.dump(value, stream, ensure_ascii=False, allow_nan=False)
                return str(path)

            args = []
            options = self._options
            if options["test_root"] is not None:
                args += [f"--test-root={options['test_root']}", f"--test-key={options['test_key']}"]
            for flag in ("app", "marker"):
                if options[flag] is not None:
                    args.append(f"--{flag}={options[flag]}")
            if options["token"] is not None:
                args.append("--marker=" + json_file(options["token"]))
            if options["authoritative"]:
                args.append("--authoritative")
            args.append(command)
            for flag, value in (("settings", settings), ("filters", filters)):
                if value is not None:
                    args += ["--" + flag, json_file(value)]
            if folder is not None:
                args.append("--folder=" + folder)
            inputs = list(positionals) if document is None else [json_file(document), *positionals]
            if inputs:
                args += ["--", *inputs]
            return execute(self.binary, args, options["timeout"])
