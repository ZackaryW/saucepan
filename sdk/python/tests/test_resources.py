import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'contracts'))
from fixtures.resource_flow import ResourceFlow
