import json
import subprocess
from pathlib import Path
from fixtures.resource_flow import ResourceFlow, BINARY


class NodeAdapter:
    def __init__(self, root, app=None):
        self.root, self.app, self.used = str(root), app, set()

    def call(self, name, *args, **options):
        self.used.add(name.replace('_', '-'))
        driver = Path(__file__).parent / 'fixtures/node_adapter.mjs'
        result = subprocess.run(['node', str(driver)], input=json.dumps(dict(binary=str(BINARY), root=self.root, app=self.app, name=name, args=args, options=options)).encode(), capture_output=True, check=True)
        value = json.loads(result.stdout)
        if 'error' in value:
            error = RuntimeError(value['error']['code'])
            error.code = value['error']['code']
            raise error
        if 'transport' in value:
            error = RuntimeError(value['transport'])
            error.kind = value['transport']
            raise error
        return value['result']


class NodeResourceFlow(ResourceFlow):
    adapter = NodeAdapter

# Avoid discovering the imported Python suite a second time.
del ResourceFlow
