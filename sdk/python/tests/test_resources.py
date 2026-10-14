import importlib.util
from pathlib import Path

fixture = Path(__file__).resolve().parents[2] / 'contracts/fixtures/resource_flow.py'
spec = importlib.util.spec_from_file_location('resource_flow', fixture)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
ResourceFlow = module.ResourceFlow
