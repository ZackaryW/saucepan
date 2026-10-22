"""Same public SDK setup scenarios run against Python and TypeScript adapters."""
import hashlib
import http.server
import json
import os
from pathlib import Path
import shutil
import threading
import uuid
import zipfile
import platform

class SetupFlowMixin:
    def test_profiles_explanations_and_managed_lifecycle(self):
        from fixtures.resource_flow import BINARY
        shim = BINARY.with_name('saucepan-shim' + ('.exe' if os.name == 'nt' else ''))
        self.assertTrue(shim.is_file(), 'build --bins before SDK contract tests')
        absent = self.base / 'absent-root'
        preview = self.adapter(absent)
        self.assertEqual(preview.call('setup_status')['state'], 'absent')
        self.assertEqual(preview.call('profiles'), ['managed','explicit-update','temporary'])
        profile = preview.call('profile', 'explicit-update', for_install=True)
        self.assertEqual(profile, preview.call('profile','explicit-update',for_install=True))
        explanation = preview.call('explain_policies', {'operation':'retrieve', 'request': {
            'source': {'kind':'inline','source':{'type':'url','url':'http://127.0.0.1:1/not-contacted'}},
            'selector':{'type':'url'},'policy_refs':profile['retrieval_policy_refs']}}, policies=profile['catalog'])
        self.assertEqual(explanation['axes']['refresh']['origin'], 'request')
        self.assertFalse(absent.exists())
        self.sdk.used.update(preview.used)
        self.call('register_policies',profile['catalog'])
        payload = self.base/'executable-source'; payload.mkdir()
        shutil.copy2(BINARY,payload/BINARY.name)
        source = self.call('register_source',{'type':'local','path':str(payload)})['uuid']
        request = {'source_uuid':source,'selector':{'type':'local'},'selection':{'bin':[BINARY.name]},'policy_refs':profile['combined_policy_refs']}
        probe = {'args':['--version'],'stdout_contains':'saucepan ','timeout_seconds':10}
        operation = str(uuid.uuid4())
        installed = self.call('install_application','SDK tool',request,probe,str(shim),scope='sdk/tool',shim='sdk-tool',operation=operation)
        self.assertEqual(installed['exit_code'],0)
        first = installed['result']; app,claim,entry=first['app'],first['claim'],first['candidate']
        # Idempotency is caller initiated. The SDK performs no retry itself.
        self.assertEqual(self.call('install_application','SDK tool',request,probe,str(shim),scope='sdk/tool',shim='sdk-tool',operation=operation),installed)
        bound = self.adapter(self.root,app)
        self.domain('access_denied',bound.call,'install_application','denied',request,probe,str(shim),scope='sdk/denied',shim='denied')
        updated = bound.call('update_application',app,claim,probe)
        self.assertEqual(updated['exit_code'],0)
        failed = bound.call('update_application',app,claim,{**probe,'stdout_contains':'impossible-sdk-probe-output'})
        self.assertEqual(failed['exit_code'],2)
        self.assertEqual(failed['result']['status'],'failed')
        self.assertEqual(bound.call('rollback',app,claim,entry,probe)['exit_code'],0)
        records = bound.call('operations',app=app)
        self.assertIn(failed['result']['operation'],[r['operation'] for r in records])
        repaired = self.call('reconcile',operation=failed['result']['operation'])
        self.assertEqual(repaired['exit_code'],2)
        self.assertEqual(repaired['result'][0]['status'],'failed')
        self.assertEqual(bound.call('uninstall',app)['exit_code'],0)
        self.sdk.used.update(bound.used)

        archive = self.base/'saucepan.zip'
        with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED) as output:
            output.write(BINARY,BINARY.name)
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        target = {('Windows','AMD64'):'x86_64-pc-windows-msvc', ('Windows','ARM64'):'aarch64-pc-windows-msvc',
                  ('Linux','x86_64'):'x86_64-unknown-linux-musl', ('Darwin','arm64'):'aarch64-apple-darwin',
                  ('Darwin','x86_64'):'x86_64-apple-darwin'}[(platform.system(), platform.machine())]
        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(inner):
                tag = inner.path.rsplit('/', 1)[-1] if '/tags/' in inner.path else 'v1'
                data = archive.read_bytes() if inner.path == '/asset' else json.dumps({
                    'id':1,'tag_name':tag,'draft':False,'prerelease':False,'assets':[
                        {'id':index,'name':name,'browser_download_url':base+'/asset','digest':'sha256:'+digest}
                        for index, name in enumerate(['saucepan.zip', f'saucepan-{tag}-{target}.zip'], 1)]}).encode()
                inner.send_response(200); inner.send_header('Content-Length',str(len(data))); inner.end_headers(); inner.wfile.write(data)
            def log_message(*_): pass
        server = http.server.ThreadingHTTPServer(('127.0.0.1',0),Handler)
        base = f'http://127.0.0.1:{server.server_port}'
        thread = threading.Thread(target=server.serve_forever,daemon=True); thread.start()
        try:
            setup = self.adapter(self.root,api=base)
            release = setup.call('register_source',{'type':'github_release','repo':'fixture/sdk'})['uuid']
            asset_filter = {'uuid':str(uuid.uuid4()),'type':'github_release_asset_filter','asset_patterns':['saucepan.zip']}
            setup.call('register_policies',[asset_filter])
            request = {'source_uuid':release,'selector':{'type':'latest_stable'},'selection':{
                'format':'zip','bin':[BINARY.name],'filter_refs':[asset_filter['uuid']]},'policy_refs':profile['combined_policy_refs']}
            boot = setup.call('bootstrap',request,str(shim))
            self.assertEqual(boot['exit_code'],0)
            app,claim=boot['result']['app'],boot['result']['claim']
            self.assertEqual(setup.call('self_update',app,claim)['exit_code'],0)
            self.assertEqual(setup.call('uninstall',app)['exit_code'],0)
            managed = setup.call('setup', str(shim), tag='v1', operation=str(uuid.uuid4()))
            self.assertEqual(managed['exit_code'], 0)
            self.assertEqual(setup.call('setup_status')['state'], 'ready')
            self.assertFalse(setup.call('setup', str(shim), tag='v1')['result']['changed'])
            self.assertEqual(setup.call('self_update', managed['result']['app'], managed['result']['claim'], tag='v2')['exit_code'], 0)
            self.assertEqual(setup.call('setup_status')['installed_version'], 'v2')
            migration_root = self.base / 'explicit migration'
            migration = self.adapter(migration_root, api=base)
            migration.call('init')
            raw = migration_root / 'bin' / BINARY.name
            shutil.copy2(BINARY, raw)
            moved = migration.call('setup', str(shim), tag='v1', operation=str(uuid.uuid4()),
                                   migrate_unmanaged=True, expected_sha256=hashlib.sha256(raw.read_bytes()).hexdigest())
            self.assertEqual(moved['exit_code'], 0)
            self.assertTrue(Path(moved['result']['backup_paths'][0]).is_file())
            self.sdk.used.update(migration.used)
            self.sdk.used.update(setup.used)
        finally:
            server.shutdown(); server.server_close(); thread.join()
