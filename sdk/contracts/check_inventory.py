"""Fail CI when a declared operation has no real-binary scenario coverage."""
import json
import os
from pathlib import Path

root=Path(__file__).resolve().parents[2]
inventory=json.loads((root/'sdk/contracts/operations.json').read_text())
expected={row['command'] for row in inventory}
if len(expected)!=len(inventory):
    raise SystemExit('duplicate operation in inventory')
for language in ['python','ts']:
    if os.environ.get('SDK_LANGUAGE','both') not in ['both',language]:
        continue
    path=root/f'target/sdk-{language}-coverage.json'
    actual=set(json.loads(path.read_text()))
    if expected-actual:
        raise SystemExit(f'{language} missing real-binary coverage: {sorted(expected-actual)}')
    print(f'{language}: {len(expected)} SDK operations exercised')
