"""Explicit isolated install: manager shim root source-folder executable-name."""
import sys
from saucepan_sdk import Client
from saucepan_sdk.models.claim import AcquireRequest
from saucepan_sdk.models.setup import Probe

binary, shim, root, source_folder, executable_name = sys.argv[1:]
client = Client(executable=binary, root=root)
client.init()  # Basic integrity; no keyring or credential setup.
profile = client.profile('explicit-update', for_install=True)
client.register_policies(profile['catalog'])
source = client.register_source({'type': 'local', 'path': source_folder})
request: AcquireRequest = {'source_uuid': source['uuid'], 'selector': {'type': 'local'},
    'selection': {'bin': [executable_name]}, 'policy_refs': profile['combined_policy_refs']}
probe: Probe = {'args': ['--version'], 'stdout_contains': 'saucepan ', 'timeout_seconds': 10}
installed = client.install_application('Example tool', request, probe, shim, scope='example/tool', shim='example-tool')
print(installed)
if installed['exit_code'] != 0:
    raise SystemExit('Inspect this operation before explicitly retrying or reconciling.')
app, claim = installed['result']['app'], installed['result']['claim']
print(client.operations(app=app))
# Cleanup here is an explicit example action, not automatic SDK compensation.
print(client.uninstall(app))
