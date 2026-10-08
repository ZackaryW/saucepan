"""Run managed-process verification in a hidden Windows console."""
import os
import subprocess
from pathlib import Path

if os.name != "nt":
    raise SystemExit("This console check requires Windows.")
info = subprocess.STARTUPINFO()
info.dwFlags = subprocess.STARTF_USESHOWWINDOW
info.wShowWindow = 0
result = subprocess.run(
    ["cargo", "test", "--test", "managed", "active_switch", "--", "--nocapture"],
    cwd=Path(__file__).resolve().parents[1],
    startupinfo=info,
    creationflags=subprocess.CREATE_NEW_CONSOLE,
    capture_output=True,
    text=True,
)
print(result.stdout)
print(result.stderr)
if result.returncode == 0:
    assert "Windows console CTRL_BREAK forwarding verified" in result.stderr
raise SystemExit(result.returncode)
