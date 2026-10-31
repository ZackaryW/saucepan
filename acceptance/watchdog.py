"""Deadlines for every invocation, with a process-tree snapshot before killing a stall."""
from __future__ import annotations

from contextlib import contextmanager
import json
import os
import signal
import subprocess
import threading
import time

DEFAULT_DEADLINE = 120.0


class Stall(AssertionError):
    """An invocation exceeded its deadline; ``report`` carries the diagnostics."""

    def __init__(self, report: dict) -> None:
        self.report = report
        lines = [f"{report['label']} stalled after {report['elapsed']:.1f}s"]
        lines += [f"  pid {p['pid']} parent {p['ppid']}: {p['command']}" for p in report["processes"]]
        if not report["processes"]:
            lines.append("  no live descendant processes")
        if report.get("snapshot_error"):
            lines.append(f"  snapshot failed: {report['snapshot_error']}")
        super().__init__("\n".join(lines))


def processes() -> list[dict]:
    """All processes as ``{pid, ppid, command}``."""
    if os.name == "nt":
        script = ("Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name,CommandLine "
                  "| ConvertTo-Json -Compress")
        output = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command", script],
                                capture_output=True, text=True, timeout=30, check=True).stdout
        rows = json.loads(output or "[]")
        return [{"pid": r["ProcessId"], "ppid": r["ParentProcessId"],
                 "command": r.get("CommandLine") or r.get("Name") or ""} for r in rows]
    output = subprocess.run(["ps", "-eo", "pid=,ppid=,args="], capture_output=True, text=True,
                            timeout=30, check=True).stdout
    rows = []
    for line in output.splitlines():
        pid, ppid, *command = line.split(None, 2)
        rows.append({"pid": int(pid), "ppid": int(ppid), "command": command[0] if command else ""})
    return rows


def descendants(root: int, table: list[dict]) -> list[dict]:
    children: dict[int, list[dict]] = {}
    for row in table:
        children.setdefault(row["ppid"], []).append(row)
    found, pending = [], [root]
    while pending:
        for row in children.get(pending.pop(), []):
            if row["pid"] != root and row not in found:
                found.append(row)
                pending.append(row["pid"])
    return found


def kill(rows: list[dict]) -> None:
    for row in rows:
        try:
            if os.name == "nt":
                subprocess.run(["taskkill", "/F", "/T", "/PID", str(row["pid"])],
                               capture_output=True, timeout=30)
            else:
                os.kill(row["pid"], signal.SIGKILL)
        except (OSError, subprocess.SubprocessError):
            pass


class Watchdog:
    def __init__(self, default: float = DEFAULT_DEADLINE) -> None:
        self.default = default

    @contextmanager
    def step(self, label: str, deadline: float | None = None):
        """Run a block; at the deadline snapshot and kill this process's descendants, then fail."""
        start = time.monotonic()
        fired: dict = {}

        def expire():
            report = {"label": label, "elapsed": time.monotonic() - start, "processes": []}
            try:
                tree = [r for r in descendants(os.getpid(), processes())
                        if "Get-CimInstance Win32_Process" not in r["command"]]
                report["processes"] = tree
            except Exception as error:  # the kill still runs without a snapshot
                report["snapshot_error"] = repr(error)
                tree = []
            fired["report"] = report
            kill(tree)

        timer = threading.Timer(deadline or self.default, expire)
        timer.daemon = True
        timer.start()
        try:
            yield
        except BaseException as error:
            timer.cancel()
            if fired:
                raise Stall(fired["report"]) from error
            raise
        timer.cancel()
        if fired:
            raise Stall(fired["report"])
