"""Shell-free process execution with bounded, independently drained streams."""
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
import os
import subprocess
import threading
import time
from typing import BinaryIO

from ..errors import TransportError


@dataclass(frozen=True)
class RunResult:
    exit_code: int | None
    signal: int | None
    stdout: bytes | None = None
    stderr: bytes | None = None


def invoke(executable: str, argv: Sequence[str], *, timeout: float | None = None,
           capture_limit: int = 16 * 1024 * 1024, capture: bool = True,
           cwd: str | None = None, env: Mapping[str, str] | None = None,
           cancel: threading.Event | None = None) -> RunResult:
    if capture_limit <= 0 or (timeout is not None and timeout <= 0):
        raise TransportError("invalid_input")
    # Windows batch launchers can invoke a shell even with shell=False.
    if os.name == "nt" and executable.lower().endswith((".bat", ".cmd")):
        raise TransportError("invalid_executable")
    if cancel is not None and cancel.is_set():
        raise TransportError("cancelled")
    try:
        process = subprocess.Popen([executable, *argv], shell=False, cwd=cwd, env=env,
                                   stdout=subprocess.PIPE if capture else None,
                                   stderr=subprocess.PIPE if capture else None)
    except FileNotFoundError:
        raise TransportError("missing_executable") from None
    except (OSError, ValueError):
        raise TransportError("spawn") from None
    buffers = [bytearray(), bytearray()]
    failure = threading.Event()
    readers: list[threading.Thread] = []

    def drain(stream: BinaryIO, buffer: bytearray) -> None:
        try:
            while chunk := stream.read1(65536):  # type: ignore[attr-defined]
                remaining = capture_limit - len(buffer)
                buffer.extend(chunk[:remaining])
                if len(chunk) > remaining:
                    failure.set()
        finally:
            stream.close()

    for stream, buffer in zip((process.stdout, process.stderr), buffers):
        if stream is not None:
            reader = threading.Thread(target=drain, args=(stream, buffer), daemon=True)
            reader.start()
            readers.append(reader)
    deadline = None if timeout is None else time.monotonic() + timeout
    reason: str | None = None
    try:
        while process.poll() is None or any(reader.is_alive() for reader in readers):
            if failure.is_set():
                reason = "output_limit"
            elif cancel is not None and cancel.is_set():
                reason = "cancelled"
            elif deadline is not None and time.monotonic() >= deadline:
                reason = "timeout"
            if reason:
                break
            time.sleep(0.01)
        if failure.is_set():
            reason = "output_limit"
        if reason:
            raise TransportError(reason, started=True)
        code = process.wait()
        return RunResult(code if code >= 0 else None, -code if code < 0 else None,
                         bytes(buffers[0]) if capture else None, bytes(buffers[1]) if capture else None)
    finally:
        if process.poll() is None:
            process.kill()
        process.wait()
        # A descendant may still own pipe handles. Never block cancellation on it
        # or claim that killing this wrapper killed all consumers.
        for reader in readers:
            reader.join(timeout=0.1)
