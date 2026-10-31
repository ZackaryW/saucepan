"""Managed setup and installer acceptance (scripts/test_setup.py) under the ``setup`` tag.

The suite keeps its own explicit roots and installer flows; it shares the forge
and runs after the harness has built the debug manager and shim.
"""
from pathlib import Path
import sys

_SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
if str(_SCRIPTS) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS))

import test_setup as _setup  # noqa: E402  (imported as a module so the base class is not collected twice)


class ManagedSetup(_setup.SetupAcceptance):
    tags = frozenset({"setup"})
