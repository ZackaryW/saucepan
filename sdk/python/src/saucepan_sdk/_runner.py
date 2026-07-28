"""Internal subprocess boundary for the saucepan executable."""

import json
import os
import subprocess
from pathlib import Path
from typing import Any, List, Union

from .errors import ConfigError, Conflict, InternalError, NotFound, SourceError


Pathish = Union[str, os.PathLike]
ERROR_TYPES = {
    1: NotFound,
    2: SourceError,
    3: ConfigError,
    4: Conflict,
    5: InternalError,
}


class CommandRunner:
    """Invoke a specific saucepan executable for one workspace."""

    def __init__(self, binary: Pathish, root: Pathish) -> None:
        self.binary = os.fspath(binary)
        self.root = Path(root)

    def run_json(self, *args: str) -> Any:
        """Run a command whose successful stdout is one JSON document."""
        return json.loads(self.run_text(*args))

    def run_ndjson(self, *args: str) -> List[Any]:
        """Run a command whose successful stdout contains JSON values by line."""
        output = self.run_text(*args)
        return [json.loads(line) for line in output.splitlines() if line.strip()]

    def run_text(self, *args: str) -> str:
        """Run a command and return its successful standard output."""
        completed = subprocess.run(
            [self.binary, str(self.root), *args],
            check=False,
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        if completed.returncode != 0:
            error_type = ERROR_TYPES.get(completed.returncode, InternalError)
            raise error_type(completed.returncode, completed.stderr)
        return completed.stdout
