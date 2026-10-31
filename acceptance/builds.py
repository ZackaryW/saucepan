"""Manager and shim binaries: built once per run, or taken from packaged release assets.

Cargo always receives the environment captured at import, before the sandbox
redirects home directories, so builds use the developer's normal toolchain state.
In artifacts mode no compiler runs: binaries come from downloaded release archives.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import subprocess
import sys
import threading
import zipfile

from . import REPO

EXE = ".exe" if os.name == "nt" else ""
ORIGINAL_ENV = dict(os.environ)
# Same platform-to-asset mapping setup uses (src/core/setup/preparation.rs).
HOST_TARGETS = {
    ("Windows", "amd64"): "x86_64-pc-windows-msvc", ("Windows", "x86_64"): "x86_64-pc-windows-msvc",
    ("Windows", "arm64"): "aarch64-pc-windows-msvc", ("Windows", "aarch64"): "aarch64-pc-windows-msvc",
    ("Windows", "x86"): "i686-pc-windows-msvc", ("Windows", "i686"): "i686-pc-windows-msvc",
    ("Darwin", "x86_64"): "x86_64-apple-darwin", ("Darwin", "arm64"): "aarch64-apple-darwin",
    ("Darwin", "aarch64"): "aarch64-apple-darwin",
    ("Linux", "x86_64"): "x86_64-unknown-linux-musl", ("Linux", "amd64"): "x86_64-unknown-linux-musl",
}
TWIN_SUFFIX = "-verify"


@dataclass(frozen=True)
class Build:
    label: str
    manager: Path
    shim: Path
    archive: Path | None
    digest: str
    build_id: str = "dev"


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def host_target() -> str:
    key = (platform.system(), platform.machine().lower())
    if key not in HOST_TARGETS:
        raise RuntimeError(f"no release target for host {key}")
    return HOST_TARGETS[key]


@dataclass(frozen=True)
class Artifacts:
    """Downloaded release assets: the candidate set, its verification twin and the previous release."""
    candidate: Path
    twin: Path | None = None
    previous: Path | None = None

    @staticmethod
    def archive(directory: Path, target: str) -> tuple[Path, str]:
        """The ``saucepan-<tag>-<target>.zip`` in ``directory`` and its tag."""
        pattern = re.compile(rf"saucepan-(?P<tag>v[^/\\]+)-{re.escape(target)}\.zip")
        found = [(p, m["tag"]) for p in directory.iterdir() if (m := pattern.fullmatch(p.name))]
        if len(found) != 1:
            raise RuntimeError(f"expected one {target} archive in {directory}, found {[p.name for p, _ in found]}")
        return found[0]

    @property
    def tag(self) -> str:
        return self.archive(self.candidate, host_target())[1]

    def packaged_targets(self) -> list[str]:
        names = [p.name for p in self.candidate.iterdir()]
        targets = set(HOST_TARGETS.values()) | {"universal-apple-darwin"}
        return sorted(t for t in targets
                      if any(n.startswith("saucepan-v") and n.endswith(f"-{t}.zip") for n in names))


def unpack(archive: Path, destination: Path, label: str, build_id: str) -> Build:
    destination.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive) as bundle:
        bundle.extractall(destination)
    manager, shim = destination / ("saucepan" + EXE), destination / ("saucepan-shim" + EXE)
    for path in (manager, shim):
        if not path.is_file():
            raise RuntimeError(f"{archive.name} lacks {path.name}")
        path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return Build(label, manager, shim, archive, digest(manager), build_id)


class Builds:
    """Memoized builds; ``cargo`` is the only command this class runs, and never in artifacts mode."""

    def __init__(self, directory: Path, cargo: list[str] | None = None,
                 artifacts: Artifacts | None = None) -> None:
        self.directory = directory
        self.cargo = cargo or ["cargo"]
        self.artifacts = artifacts
        self.invocations: list[tuple[list[str], str | None]] = []
        self._cache: dict[str, Build] = {}
        self._release_order: list[str] = []
        self._lock = threading.Lock()

    def _cargo(self, args: list[str], env: dict[str, str]) -> None:
        if self.artifacts is not None:
            raise RuntimeError("artifacts mode never compiles")
        command = [*self.cargo, *args]
        self.invocations.append((command, env.get("SAUCEPAN_BUILD_ID")))
        subprocess.run(command, cwd=REPO, env=env, check=True, stdout=sys.stderr)

    def _candidate(self) -> Build:
        if "candidate" not in self._cache:
            archive, tag = Artifacts.archive(self.artifacts.candidate, host_target())
            self._cache["candidate"] = unpack(archive, self.directory / "candidate", "candidate", tag)
        return self._cache["candidate"]

    def _twin(self) -> Build:
        if "twin" not in self._cache:
            if self.artifacts.twin is None:
                raise RuntimeError("the lifecycle scenario needs --twin in artifacts mode")
            target = host_target()
            archive = self.artifacts.twin / f"verify-twin-{target}.zip"
            if not archive.is_file():
                raise RuntimeError(f"missing {archive}")
            self._cache["twin"] = unpack(archive, self.directory / "twin", "twin",
                                         self._candidate().build_id + TWIN_SUFFIX)
        return self._cache["twin"]

    def previous(self) -> Build | None:
        """The previous published release's host archive, unpacked, when provided."""
        with self._lock:
            if self.artifacts is None or self.artifacts.previous is None:
                return None
            if "previous" not in self._cache:
                archive, tag = Artifacts.archive(self.artifacts.previous, host_target())
                self._cache["previous"] = unpack(archive, self.directory / "previous", "previous", tag)
            return self._cache["previous"]

    def manager(self) -> Build:
        """Debug manager and shim from the checkout, or the candidate's packaged binaries."""
        with self._lock:
            if self.artifacts is not None:
                return self._candidate()
            if "debug" not in self._cache:
                self._cargo(["build", "--locked", "--bins"], dict(ORIGINAL_ENV))
                target = REPO / "target" / "debug"
                manager = target / ("saucepan" + EXE)
                self._cache["debug"] = Build("debug", manager, target / ("saucepan-shim" + EXE),
                                             None, digest(manager))
            return self._cache["debug"]

    def release(self, label: str) -> Build:
        """A distinguishable release build. From source: stamped with ``label``. In artifacts
        mode: the first label requested is the candidate and the second its verification twin."""
        with self._lock:
            if self.artifacts is not None:
                if label not in self._release_order:
                    self._release_order.append(label)
                position = self._release_order.index(label)
                if position > 1:
                    raise RuntimeError("artifacts mode provides two distinguishable builds")
                return self._candidate() if position == 0 else self._twin()
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
                self._cache[label] = Build(label, manager, shim, archive, digest(manager), label)
            return self._cache[label]

    def built(self) -> list[Build]:
        return list(self._cache.values())
