"""Run-wide home isolation and a fresh, explicit root per scenario.

``Run.isolate()`` points every home variable at the run directory so the manager,
shims and consumers can never resolve the developer's real ``~/.saucepan``.
``Sandbox`` gives one scenario its root, clock, forge, SDK client and the typed
``admin`` adapter, the only place that spells commands the SDK does not expose.
"""
from __future__ import annotations

from dataclasses import replace
import json
import os
from pathlib import Path
import subprocess
import uuid

from . import REPO
from .builds import Builds
from .forge import Forge
from .watchdog import Watchdog

HOME_VARIABLES = ("HOME", "USERPROFILE", "XDG_CONFIG_HOME", "XDG_CACHE_HOME",
                  "XDG_DATA_HOME", "APPDATA", "LOCALAPPDATA")


class Run:
    def __init__(self, directory: Path | None = None) -> None:
        self.id = str(uuid.uuid4())
        self.dir = directory or REPO / "target" / "acceptance" / self.id
        self.home = self.dir / "home"
        self.builds = Builds(self.dir / "builds")
        self.watchdog = Watchdog()
        self._saved: dict[str, str | None] | None = None

    def isolate(self) -> None:
        self._saved = point_home(self.home)

    def restore(self) -> None:
        restore_home(self._saved or {})
        self._saved = None


def point_home(home: Path) -> dict[str, str | None]:
    """Point every home variable beneath ``home``; returns the previous values."""
    saved = {name: os.environ.get(name) for name in HOME_VARIABLES}
    for name in HOME_VARIABLES:
        path = home if name in ("HOME", "USERPROFILE") else home / name.lower()
        path.mkdir(parents=True, exist_ok=True)
        os.environ[name] = str(path)
    return saved


def restore_home(saved: dict[str, str | None]) -> None:
    for name, value in saved.items():
        if value is None:
            os.environ.pop(name, None)
        else:
            os.environ[name] = value


class Clock:
    def __init__(self, now: int = 100) -> None:
        self.now = now

    def advance(self, *, days: float = 0, seconds: int = 0) -> int:
        self.now += int(days * 86400) + seconds
        return self.now


class Sandbox:
    def __init__(self, run: Run, name: str, *, gh_token: str | None = None) -> None:
        self.run = run
        self.dir = run.dir / "scenarios" / name
        self.dir.mkdir(parents=True, exist_ok=False)
        self.root = self.dir / "root"
        # A home per scenario: a default root one scenario creates is invisible to the next.
        self.home = self.dir / "home"
        self._saved_home = point_home(self.home)
        self.clock = Clock()
        self.forge = Forge().start()
        self.manager = run.builds.manager().manager
        self.gh = _fake_gh(self.dir, gh_token) if gh_token else None
        self._children: list[subprocess.Popen] = []
        self.client = self.client_for(self.manager)
        self.admin = Admin(self)
        self.client.init()

    # -- manager access ------------------------------------------------------
    def global_args(self) -> list[str]:
        args = ["--test-now", str(self.clock.now),
                "--test-github-api", self.forge.github_api,
                "--test-github-raw", self.forge.github_raw,
                "--test-git-raw", self.forge.git_raw]
        if self.gh:
            args += ["--test-gh", str(self.gh)]
        return args

    def client_for(self, executable: Path, *, app: str | None = None):
        from saucepan_sdk import Client
        from saucepan_sdk.transport import CliTransport

        sandbox = self

        class SandboxTransport(CliTransport):
            def _invoke(self, config, call, options):
                call = replace(call, argv=[*sandbox.global_args(), *call.argv])
                label = " ".join(str(a) for a in call.argv if isinstance(a, str) and not a.startswith("http"))
                with sandbox.run.watchdog.step(f"sdk: {label}"):
                    return super()._invoke(config, call, options)

        client = Client(executable=str(executable), root=str(self.root),
                        timeout=self.run.watchdog.default + 30, transport=SandboxTransport())
        return client.for_app(app) if app else client

    def payload(self, name: str, value: object) -> Path:
        path = self.dir / f"{name}-{uuid.uuid4().hex[:8]}.json"
        path.write_text(json.dumps(value), encoding="utf-8")
        return path

    def cli(self, *args: object, executable: Path | None = None, code: int = 0,
            raw: bool = False, root: bool = True, deadline: float | None = None):
        """Literal argv with the sandbox root and endpoints; returns parsed stdout JSON."""
        command = [str(executable or self.manager)]
        if root:
            command += ["--root", str(self.root)]
        command += [*self.global_args(), *map(str, args)]
        result = self.launch(*command, deadline=deadline)
        assert result.returncode == code, (args, result.returncode, result.stdout, result.stderr)
        if raw:
            return result
        return json.loads(result.stdout) if result.stdout.strip() else None

    def launch(self, *command: object, deadline: float | None = None) -> subprocess.CompletedProcess:
        """Run any executable (shims, consumers) under the watchdog without adding arguments."""
        argv = list(map(str, command))
        with self.run.watchdog.step("launch: " + " ".join(argv[:4]), deadline):
            return subprocess.run(argv, cwd=self.dir, capture_output=True, text=True,
                                  timeout=(deadline or self.run.watchdog.default) + 30)

    def background(self, *command: object) -> subprocess.Popen:
        """A long-lived child (for example a busy launcher), stopped when the sandbox closes."""
        process = subprocess.Popen(list(map(str, command)), cwd=self.dir, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, text=True)
        self._children.append(process)
        return process

    def close(self) -> None:
        for process in self._children:
            if process.poll() is None:
                process.kill()
                process.communicate(timeout=30)
        self.forge.close()
        restore_home(self._saved_home)


class Admin:
    """Commands the SDK deliberately omits; the only place their argv is spelled."""

    def __init__(self, sandbox: Sandbox) -> None:
        self.sandbox = sandbox

    def register_app(self, name: str) -> dict:
        return self.sandbox.cli("register-app", name)

    def activate(self, app: str, entry: str, name: str, probe: dict) -> dict:
        return self.sandbox.cli("activate", app, entry, name, self.sandbox.payload("probe", probe))

    def deactivate(self, app: str) -> dict:
        return self.sandbox.cli("deactivate", app)

    def install_shim(self, app: str, name: str, shim: Path, *, saucepan: bool = False) -> dict:
        return self.sandbox.cli("install-shim", app, name, shim, *(["--saucepan"] if saucepan else []))

    def replace_shim(self, app: str, name: str, shim: Path, *, saucepan: bool = False, code: int = 0) -> dict:
        return self.sandbox.cli("replace-shim", app, name, shim, *(["--saucepan"] if saucepan else []), code=code)


def _fake_gh(directory: Path, token: str) -> Path:
    if os.name == "nt":
        path = directory / "gh.cmd"
        path.write_text(f"@echo off\r\necho {token}\r\n", encoding="utf-8")
    else:
        path = directory / "gh"
        path.write_text(f"#!/bin/sh\necho {token}\n", encoding="utf-8")
        path.chmod(0o755)
    return path
