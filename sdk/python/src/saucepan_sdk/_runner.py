"""Shell-free execution; command lines may contain test secrets, never echo them."""
import json
import subprocess
from typing import Any, Optional, Sequence

from .errors import SaucepanError


def execute(binary: str, arguments: Sequence[str], timeout: Optional[float]) -> Any:
    try:
        result = subprocess.run(
            [binary, *arguments], capture_output=True, shell=False,
            encoding="utf-8", errors="replace", timeout=timeout,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        )
    except subprocess.TimeoutExpired as error:
        def text(value):
            return value.decode("utf-8", errors="replace") if isinstance(value, bytes) else value or ""
        raise SaucepanError("Saucepan timed out", stdout=text(error.stdout),
                            stderr=text(error.stderr), code="TIMEOUT") from None
    except OSError as error:
        raise SaucepanError(f"Could not execute Saucepan: {binary}",
                            code=type(error).__name__) from None
    if result.returncode:
        raise SaucepanError(f"Saucepan exited with code {result.returncode}: {result.stderr.strip()}",
                            result.returncode, result.stdout, result.stderr)
    try:
        return json.loads(result.stdout)
    except ValueError:
        raise SaucepanError("Saucepan returned invalid JSON", 0, result.stdout,
                            result.stderr, "INVALID_JSON") from None
