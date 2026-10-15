"""Copied outside the checkout to exercise only the installed public SDK."""
import json
from pathlib import Path
import shutil
import sys
from saucepan_sdk import Client

binary, shim, root = map(Path, sys.argv[1:])
client = Client(executable=str(binary), root=str(root))
assert 'managed' in client.profiles()
profile = client.profile('explicit-update', for_install=True)
explained = client.explain_policies({'operation':'retrieve', 'request': {
    'source':{'kind':'inline','source':{'type':'url','url':'http://127.0.0.1:1/unused'}},
    'selector':{'type':'url'},'policy_refs':profile['retrieval_policy_refs']}}, policies=profile['catalog'])
assert explained['axes']['refresh']['origin'] == 'request'
assert not root.exists()
client.init()
client.register_policies(profile['catalog'])
folder = root.parent/'payload'; folder.mkdir()
shutil.copy2(binary, folder/binary.name)
source = client.register_source({'type':'local','path':str(folder)})
request = {'source_uuid':source['uuid'],'selector':{'type':'local'},
           'selection':{'bin':[binary.name]},'policy_refs':profile['combined_policy_refs']}
probe = {'args':['--version'],'stdout_contains':'saucepan ','timeout_seconds':10}
installed = client.install_application('Packaged SDK',request,probe,str(shim),scope='package/tool',shim='packaged-tool')
assert installed['exit_code'] == 0
record = installed['result']; app,claim,entry=record['app'],record['claim'],record['candidate']
assert client.update_application(app,claim,probe)['exit_code'] == 0
failed = client.update_application(app,claim,{**probe,'stdout_contains':'impossible-package-probe'})
assert failed['exit_code'] == 2 and failed['result']['status'] == 'failed'
assert client.rollback(app,claim,entry,probe)['exit_code'] == 0
assert any(r['operation'] == failed['result']['operation'] for r in client.operations(app=app))
assert client.reconcile(operation=failed['result']['operation'])['exit_code'] == 2
assert client.uninstall(app)['exit_code'] == 0
print(json.dumps({'package':'python','profile_digest':profile['digest'],'install':'complete','failed_probe':failed['result']['status'],'rollback':'complete','uninstall':'complete'}))
