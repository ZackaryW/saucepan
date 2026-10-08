"""Run managed-process verification in a hidden Windows console."""
import os
import subprocess
from pathlib import Path

if os.name != "nt":
    raise SystemExit("This console check requires Windows.")
info = subprocess.STARTUPINFO()
info.dwFlags = subprocess.STARTF_USESHOWWINDOW
info.wShowWindow = 0
for suite, test, evidence in [
    ("managed", "active_switch", "Windows console CTRL_BREAK forwarding verified"),
    ("run", "consumer_console", "Leased run console/signal interruption verified"),
]:
    result = subprocess.run(
        ["cargo", "test", "--test", suite, test, "--", "--nocapture"],
        cwd=Path(__file__).resolve().parents[1],
        startupinfo=info,
        creationflags=subprocess.CREATE_NEW_CONSOLE,
        capture_output=True,
        text=True,
    )
    print(result.stdout)
    print(result.stderr)
    if result.returncode:
        raise SystemExit(result.returncode)
    assert evidence in result.stderr
