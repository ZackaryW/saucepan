"""Bounded portable JSON and per-call owned files."""
from collections.abc import Iterator, Mapping, Sequence
from contextlib import contextmanager
import json
import math
from pathlib import Path
from tempfile import TemporaryDirectory

from ..errors import TransportError

MAX_INTEGER = 2**53 - 1


def portable(value: object) -> None:
    if value is None or isinstance(value, (str, bool)):
        return
    if isinstance(value, int):
        if abs(value) > MAX_INTEGER:
            raise ValueError("integer outside portable range")
        return
    if isinstance(value, float):
        if not math.isfinite(value) or abs(value) > MAX_INTEGER:
            raise ValueError("number outside portable range")
        return
    if isinstance(value, list):
        for item in value:
            portable(item)
        return
    if isinstance(value, dict):
        for key, item in value.items():
            if not isinstance(key, str):
                raise ValueError("JSON keys must be strings")
            portable(item)
        return
    raise ValueError("value is not JSON data")


def encode(value: object, limit: int) -> bytes:
    try:
        portable(value)
        data = json.dumps(value, ensure_ascii=False, allow_nan=False, separators=(",", ":")).encode("utf-8")
        if len(data) > limit:
            raise TransportError("input_limit")
        return data
    except (ValueError, TypeError, RecursionError, UnicodeError):
        raise TransportError("invalid_input") from None


def decode(data: bytes) -> object:
    try:
        result: object = json.loads(data.decode("utf-8"))
        portable(result)
        return result
    except (ValueError, TypeError, RecursionError, UnicodeError):
        raise TransportError("protocol", started=True) from None


@contextmanager
def payload_files(values: Mapping[str, object], *, limit: int) -> Iterator[dict[str, str]]:
    # Encode before any filesystem effects; no caller mutation or open handles.
    encoded = {name: encode(value, limit) for name, value in values.items()}
    with TemporaryDirectory(prefix="saucepan-sdk-") as directory:
        paths: dict[str, str] = {}
        for index, (name, data) in enumerate(encoded.items()):
            path = Path(directory) / f"{index}.json"
            path.write_bytes(data)
            paths[name] = str(path)
        yield paths
