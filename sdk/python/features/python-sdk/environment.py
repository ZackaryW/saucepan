import sys
from pathlib import Path


SDK_ROOT = Path(__file__).resolve().parents[2]
if str(SDK_ROOT) not in sys.path:
    sys.path.insert(0, str(SDK_ROOT))

from features.support.lifecycle import after_scenario, before_all, before_scenario
