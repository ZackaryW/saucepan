"""Build manager and shim binaries once per run.

Cargo always receives the environment captured at import, before the sandbox
redirects home directories, so builds use the developer's normal toolchain state.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import threading
import zipfile

from . import REPO

EXE = ".exe" if os.name == "nt" else ""
ORIGINAL_ENV = dict(os.environ)


@dataclass(frozen=True)
class Build:
    label: str
    manager: Path
    shim: Path
    archive: Path | None
    digest: str


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


class Builds:
    """Memoized builds; ``cargo`` is the only command this class runs."""

    def __init__(self, directory: Path, cargo: list[str] | None = None) -> None:
        self.directory = directory
        self.cargo = cargo or ["cargo"]
        self.invocations: list[tuple[list[str], str | None]] = []
        self._cache: dict[str, Build] = {}
        self._lock = threading.Lock()

    def _cargo(self, args: list[str], env: dict[str, str]) -> None:
        command = [*self.cargo, *args]
        self.invocations.append((command, env.get("SAUCEPAN_BUILD_ID")))
        subprocess.run(command, cwd=REPO, env=env, check=True, stdout=sys.stderr)

    def manager(self) -> Build:
        """Debug manager and shim from the checkout."""
        with self._lock:
            if "debug" not in self._cache:
                self._cargo(["build", "--locked", "--bins"], dict(ORIGINAL_ENV))
                target = REPO / "target" / "debug"
                manager = target / ("saucepan" + EXE)
                self._cache["debug"] = Build("debug", manager, target / ("saucepan-shim" + EXE),
                                             None, digest(manager))
            return self._cache["debug"]

    def release(self, label: str) -> Build:
        """Release build stamped with ``label``, copied out and packed as ``saucepan.zip``."""
        with self._lock:
            if label not in self._cache:
                self._cargo(["build", "--locked", "--release", "--bins"], dict(ORIGINAL_ENV, SAUCEPAN_BUILD_ID=label))
                target = REPO / "target" / "release"
                directory = self.directory / label
                directory.mkdir(parents=True, exist_ok=True)
                manager = directory / ("saucepan" + EXE)
                shim = directory / ("saucepan-shim" + EXE)
                shutil.copy2(target / manager.name, manager)
                shutil.copy2(target / shim.name, shim)
                archive = directory / "saucepan.zip"
                with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
                    bundle.write(manager, manager.name)
                self._cache[label] = Build(label, manager, shim, archive, digest(manager))
            return self._cache[label]

    def built(self) -> list[Build]:
        return list(self._cache.values())
