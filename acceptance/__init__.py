"""Real-build acceptance harness: shared provider fixture, sandbox, watchdog and runner.

Run with ``python -m acceptance``. The in-repository Python SDK is imported from
``sdk/python/src`` so scenarios exercise the checkout's SDK, not an installed one.
"""
from pathlib import Path
import sys

REPO = Path(__file__).resolve().parents[1]
_SDK = REPO / "sdk" / "python" / "src"
if str(_SDK) not in sys.path:
    sys.path.insert(0, str(_SDK))
