"""Current CLI fixtures using explicit test stores; never the user's keyring."""

import json
import os
import subprocess
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class CentralTestStore:
    binary: Path
    root: Path
    key: str = field(repr=False)

    @classmethod
    def create(cls, binary: Path, directory: Path) -> "CentralTestStore":
        directory.mkdir()
        store = cls(binary, directory / "store", os.urandom(32).hex())
        store.run("init", check=True)
        return store

    def run(self, *arguments: str, check: bool = False) -> subprocess.CompletedProcess:
        result = subprocess.run(
            [str(self.binary), "--test-root", str(self.root), "--test-key", self.key, *arguments],
            capture_output=True, check=False, timeout=30,
        )
        if check and result.returncode:
            raise AssertionError(result.stderr.decode("utf-8", errors="replace"))
        return result

    def json(self, *arguments: str):
        return json.loads(self.run(*arguments, check=True).stdout)
