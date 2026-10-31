"""Machine-readable run report: environment actually used and per-scenario outcomes."""
from __future__ import annotations

import json
import os
from pathlib import Path
import platform
import shutil
import sys
import time
import traceback
import unittest

from .builds import Artifacts, Build, digest, host_target


def launch_environment() -> str:
    explicit = os.environ.get("SAUCEPAN_ACCEPTANCE_SHELL")
    if explicit:
        return explicit
    if os.environ.get("MSYSTEM"):
        return "git-bash"
    return "windows-native" if os.name == "nt" else "posix-shell"


def environment() -> dict:
    return {
        "platform": sys.platform,
        "machine": platform.machine(),
        "python": platform.python_version(),
        "launch_environment": launch_environment(),
        # The Git Bash hang depended on which git binary the manager resolved.
        "git": shutil.which("git"),
        "native_keyring": "not_run",
    }


class Outcomes(unittest.TestResult):
    def __init__(self) -> None:
        super().__init__()
        self.results: dict[str, dict] = {}
        self._started: dict[str, float] = {}

    def startTest(self, test):
        super().startTest(test)
        self._started[test.id()] = time.monotonic()
        print(f"... {test.id()}", file=sys.stderr, flush=True)

    def _record(self, test, outcome, detail=None):
        elapsed = time.monotonic() - self._started.get(test.id(), time.monotonic())
        entry = {"outcome": outcome, "seconds": round(elapsed, 1)}
        if detail is not None:
            entry["detail"] = detail
        self.results[test.id()] = entry
        print(f"{outcome:>7} {test.id()} ({elapsed:.1f}s)", file=sys.stderr, flush=True)

    def addSuccess(self, test):
        super().addSuccess(test)
        self._record(test, "passed")

    def addFailure(self, test, err):
        super().addFailure(test, err)
        self._failed(test, err)

    def addError(self, test, err):
        super().addError(test, err)
        self._failed(test, err)

    def addSkip(self, test, reason):
        super().addSkip(test, reason)
        self._record(test, "skipped", reason)

    def _failed(self, test, err):
        _, error, _ = err
        detail = {"message": str(error), "traceback": "".join(traceback.format_exception(*err))}
        if getattr(error, "report", None) is not None:
            detail["stall"] = error.report
        self._record(test, "failed", detail)


def artifacts_section(artifacts: Artifacts | None) -> dict | None:
    """Which packaged assets were tested, and which packaged targets were not."""
    if artifacts is None:
        return None
    host = host_target()
    section = {
        "candidate": {"tag": artifacts.tag,
                      "archives": {p.name: digest(p) for p in sorted(artifacts.candidate.glob("saucepan-*.zip"))}},
        "verified_target": host,
        "unverified_targets": [t for t in artifacts.packaged_targets() if t != host],
    }
    if artifacts.twin is not None:
        section["twin"] = {p.name: digest(p) for p in sorted(artifacts.twin.glob("verify-twin-*.zip"))}
    if artifacts.previous is not None:
        archive, tag = Artifacts.archive(artifacts.previous, host)
        section["previous"] = {"tag": tag, "archive": archive.name, "sha256": digest(archive)}
    return section


def write(directory: Path, run_id: str, outcomes: dict[str, dict], builds: list[Build],
          artifacts: Artifacts | None = None) -> dict:
    report = {
        "run": run_id,
        "environment": environment(),
        "artifacts": artifacts_section(artifacts),
        "builds": [{"label": b.label, "manager": str(b.manager), "sha256": b.digest,
                    "build_id": b.build_id} for b in builds],
        "scenarios": outcomes,
        "summary": {state: sum(1 for o in outcomes.values() if o["outcome"] == state)
                    for state in ("passed", "failed", "skipped", "not_selected")},
    }
    text = json.dumps(report, indent=2, default=str)
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "report.json").write_text(text, encoding="utf-8")
    (directory.parent / "latest.json").write_text(text, encoding="utf-8")
    return report
