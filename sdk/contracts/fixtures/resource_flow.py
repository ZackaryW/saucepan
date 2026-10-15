"""Shared real-binary scenarios; adapters provide only call(name, args, options)."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import uuid
from fixtures.setup_flow import SetupFlowMixin

REPO = Path(__file__).resolve().parents[3]
BINARY = Path(os.environ.get('SAUCEPAN_TEST_BINARY', REPO / 'target/debug' / ('saucepan.exe' if os.name == 'nt' else 'saucepan'))).resolve()


class PythonAdapter:
    def __init__(self, root, app=None, api=None):
        from saucepan_sdk import Client
        from saucepan_sdk.transport import CliTransport
        from dataclasses import replace
        class FixtureTransport(CliTransport):
            def _invoke(self, config, call, options):
                if api:
                    call = replace(call, argv=['--test-github-api', api, *call.argv])
                return super()._invoke(config, call, options)
        self.client = Client(executable=str(BINARY), root=str(root), transport=FixtureTransport())
        if app:
            self.client = self.client.for_app(app)
        self.used = set()

    def call(self, name, *args, **options):
        from saucepan_sdk import RunOptions, RunResult
        if 'options' in options:
            options['options'] = RunOptions(**options['options'])
        self.used.add({'install_application': 'install', 'update_application': 'update-app'}.get(name, name.replace('_', '-')))
        result = getattr(self.client, name)(*args, **options)
        if isinstance(result, RunResult):
            return dict(exit_code=result.exit_code, signal=result.signal,
                        stdout=None if result.stdout is None else result.stdout.decode('utf-8'),
                        stderr=None if result.stderr is None else result.stderr.decode('utf-8'))
        return result


class ResourceFlow(SetupFlowMixin, unittest.TestCase):
    adapter = PythonAdapter

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='saucepan-sdk-flow-')
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.root = self.base / 'root'
        self.sdk = self.adapter(self.root)
        self.call = self.sdk.call
        self.call('init')
        self.folder = self.base / 'source with 雪'
        self.folder.mkdir()
        (self.folder / 'plugin.txt').write_text('A')
        (self.folder / 'nested').mkdir()
        (self.folder / 'nested/config.txt').write_text('config')
        self.source = self.call('register_source', {'type': 'local', 'path': str(self.folder)})['uuid']
        self.request = {'source_uuid': self.source, 'selector': {'type': 'local'}}
        self.pin_policy = str(uuid.uuid4())
        self.refresh_policy = str(uuid.uuid4())
        self.call('register_policies', [
            {'uuid': self.pin_policy, 'type': 'pin_policy', 'retention': 'until_released'},
            {'uuid': self.refresh_policy, 'type': 'refresh_policy', 'mode': 'always'},
        ])

    def domain(self, code, function, *args, **options):
        try:
            function(*args, **options)
        except Exception as error:
            self.assertEqual(getattr(error, 'code', None), code, str(error))
        else:
            self.fail('expected '+code)

    def child(self, code='import os; print(os.environ["SAUCEPAN_RESOURCE_DIR"])'):
        return [sys.executable, '-c', code]

    def test_sources_policies_claims(self):
        self.assertEqual(self.call('query_sources')['sources'], [])
        visible = self.call('set_source_visibility', self.source, 'public')
        query = self.call('query_sources', {'uuid': self.source})
        self.assertEqual(query['sources'][0]['uuid'], self.source)
        direct = subprocess.run([str(BINARY), '--root', str(self.root), '--error-format', 'json', 'query-sources'], capture_output=True, check=True)
        self.assertEqual(json.loads(direct.stdout), self.call('query_sources'))
        self.assertEqual(self.call('policy', self.pin_policy)['retention'], 'until_released')
        self.call('adopt_source_policies', self.source, visible['policy_revision'], [self.refresh_policy])
        app = self.call('register_application', {'registration_key': 'consumer', 'name': 'Consumer'})
        app = self.call('configure_application', app['uuid'], {'expected_revision': app['revision'], 'policy_refs': []})
        adopted = self.call('adopt_app_policies', app['uuid'], app['revision'], [])
        bound = self.adapter(self.root, app['uuid'])
        claim = bound.call('ensure_claim', app['uuid'], {**self.request, 'scope': 'project-a', 'context': {'project': 'a'}})
        again = bound.call('ensure_claim', app['uuid'], {**self.request, 'scope': 'project-a', 'context': {'project': 'a'}})
        self.assertEqual(claim['claim']['uuid'], again['claim']['uuid'])
        b = bound.call('acquire', self.request, scope='project-b')
        self.assertNotEqual(b['claim']['uuid'], claim['claim']['uuid'])
        self.assertEqual(len(bound.call('claims', app['uuid'])), 2)
        results = bound.call('query_claims', {'app': app['uuid'], 'scope': 'project-a', 'where': []})
        self.assertEqual(len(results['claims']), 1)
        self.assertEqual(self.call('discover_claims', {'where': []}), [])
        context = bound.call('set_claim_context', app['uuid'], claim['claim']['uuid'], {'project': 'changed'}, results['revision'])
        self.assertEqual(context['context'], {'project': 'changed'})
        rev = bound.call('query_claims', {'app': app['uuid'], 'where': []})['revision']
        bound.call('adopt_claim_policies', app['uuid'], claim['claim']['uuid'], rev, [])
        self.domain('revision_conflict', bound.call, 'set_claim_context', app['uuid'], claim['claim']['uuid'], {}, 999)
        (self.folder / 'plugin.txt').write_text('B')
        prior = bound.call('query_claims', {'app': app['uuid'], 'scope': 'project-a', 'where': []})
        self.assertEqual(prior['claims'][0]['claim']['entry_uuid'], claim['claim']['entry_uuid'])
        moved = bound.call('update_claim', app['uuid'], claim['claim']['uuid'])
        self.assertNotEqual(moved['claim']['entry_uuid'], claim['claim']['entry_uuid'])
        self.assertEqual(bound.call('recover_claim', app['uuid'], claim['claim']['uuid'])['claim']['uuid'], claim['claim']['uuid'])
        run = bound.call('run_claim', app['uuid'], claim['claim']['uuid'], self.child(), options={'capture': True})
        self.assertEqual(run['exit_code'], 0)
        bound.call('release_claim', app['uuid'], claim['claim']['uuid'])
        denied_policy = str(uuid.uuid4())
        self.call('register_policies', [{'uuid': denied_policy, 'type': 'access_policy', 'sources': []}])
        denied = self.call('register_application', {'registration_key': 'denied', 'name': 'Denied', 'policy_refs': [denied_policy]})
        self.domain('access_denied', self.adapter(self.root, denied['uuid']).call, 'ensure_claim', denied['uuid'], {**self.request, 'scope': 'project-a'})
        self.sdk.used.update(bound.used)

    def test_groups_pins_and_runs(self):
        acquired = self.call('acquire', self.request)
        output = acquired['entry']['uuid']
        self.assertEqual(self.call('inspect', output)['type'], 'output')
        self.assertEqual(self.call('path', output)['directory'], acquired['directory'])
        self.assertEqual(self.call('verify', output)['verification'], 'full')
        pin = self.call('pin_outputs', {'registration_key': 'consumer-a', 'outputs': [output], 'policy_refs': [self.pin_policy]})
        self.assertTrue(self.call('pin', pin['pin']['uuid'])['outputs'][output]['available'])
        self.assertEqual(len(self.call('pins', key='consumer-a')), 1)
        group_request = {'registration_key': 'bundle', 'members': [{'name': 'plugin', 'target': {'type': 'acquire', **self.request}}]}
        group = self.call('register_group', group_request)
        self.assertEqual(self.call('show_group', group['uuid'])['uuid'], group['uuid'])
        self.assertEqual(len(self.call('list_groups', key='bundle')), 1)
        group = self.call('replace_group', group['uuid'], group_request, group['revision'])
        prepared = self.call('prepare_group', group['uuid'], expected_revision=group['revision'], pin={'registration_key': 'generation-a', 'policy_refs': [self.pin_policy]})
        retry = self.call('prepare_group', group['uuid'], expected_revision=group['revision'], pin={'registration_key': 'generation-a', 'policy_refs': [self.pin_policy]})
        self.assertEqual(retry['pin']['uuid'], prepared['pin']['uuid'])
        self.domain('revision_conflict', self.call, 'remove_group', group['uuid'], 999)
        independent = self.call('pin_group', group['uuid'], prepared['group']['revision'], {'registration_key': 'generation-independent', 'policy_refs': [self.pin_policy]})
        inline = {'members': [{'name': 'plugin', 'target': {'type': 'output', 'uuid': output}}]}
        run = self.call('run', output, self.child('import json,sys; print(json.dumps(sys.argv[1:])); sys.stderr.write(\'{"error":{"code":"source_error"}}\'); sys.exit(7)')+['雪 space', '&; $(no)'], options={'capture': True})
        self.assertEqual(run['exit_code'], 7)
        self.assertEqual(json.loads(run['stdout']), ['雪 space', '&; $(no)'])
        script = self.child('import json,os; print(len(json.load(open(os.environ["SAUCEPAN_RESOURCE_GROUP_FILE"]))["members"]))')
        for method, args in [('run_group', [inline]), ('run_registered_group', [group['uuid']]), ('prepare_run_group', [group['uuid']])]:
            self.assertEqual(self.call(method, *args, script, options={'capture': True})['exit_code'], 0)
        self.call('reclaim')
        self.assertTrue(Path(acquired['directory']).exists())
        self.assertEqual(self.call('unpin', pin['pin']['uuid'])['state'], 'released')
        self.call('unpin', independent['uuid'])
        self.call('unpin', prepared['pin']['uuid'])
        current = self.call('show_group', group['uuid'])
        self.call('remove_group', group['uuid'], current['revision'])

    def test_retrieval_and_root_free_composition(self):
        request = {'source': {'kind': 'registered', 'source_uuid': self.source}, 'selector': {'type': 'local'}}
        receipt = self.call('retrieve', request)
        retrieval = receipt['descriptor']['ownership']['uuid']
        self.assertEqual(self.call('source_path', retrieval)['path'], receipt['descriptor']['path'])
        acquired = self.call('acquire_retrieved', {'retrieval': {'mode': 'managed', 'uuid': retrieval}, 'acquisition': {'folder': 'nested'}})
        self.assertTrue((Path(acquired['path']) / 'config.txt').exists())
        self.assertEqual(self.call('run_retrieved', request, self.child('import os; print(os.environ["SAUCEPAN_SOURCE_PATH"])'), options={'capture': True})['exit_code'], 0)
        self.assertEqual(self.call('run_acquired', {'retrieval': request, 'acquisition': {'destination': {'mode': 'temporary'}}}, self.child(), options={'capture': True})['exit_code'], 0)
        self.assertTrue(self.call('purge_retrieval', retrieval)['purged'])
        policy = {'uuid': str(uuid.uuid4()), 'type': 'cache_policy', 'mode': 'no_store'}
        fresh = {'source': {'kind': 'inline', 'source': {'type': 'local', 'path': str(self.folder)}}, 'selector': {'type': 'local'}, 'destination': str(self.base / 'fresh-source'), 'policy_refs': [policy['uuid']]}
        missing_root = self.base / 'must-not-exist'
        portable = self.adapter(missing_root)
        receipt = portable.call('retrieve', fresh, policies=[policy])
        result = portable.call('acquire_retrieved', {'retrieval': {'mode': 'caller', **receipt}, 'acquisition': {'destination': {'mode': 'directory', 'path': str(self.base / 'owned')}, 'folder': 'nested'}}, policies=[policy])
        self.assertTrue((Path(result['path']) / 'config.txt').exists())
        self.assertFalse(missing_root.exists())

    def test_git_subpaths_and_atomic_group_failure(self):
        repo = self.base / 'git-origin'
        repo.mkdir()
        def git(*args):
            subprocess.run(['git', '-C', str(repo), *args], check=True, capture_output=True)
        git('init')
        git('config', 'user.name', 'SDK fixture')
        git('config', 'user.email', 'sdk@example.invalid')
        (repo / 'one').mkdir()
        (repo / 'one/tool.txt').write_text('A')
        git('add', '.')
        git('commit', '-m', 'A')
        source = self.call('register_source', {'type': 'git', 'origin': str(repo)})
        self.call('adopt_source_policies', source['uuid'], source['policy_revision'], [self.refresh_policy])
        request = {'source': {'kind': 'registered', 'source_uuid': source['uuid']}, 'selector': {'type': 'ref', 'reference': 'HEAD'}}
        receipt = self.call('retrieve', request)
        self.assertEqual(receipt['descriptor']['kind']['kind'], 'git_repository')
        acquired = self.call('acquire_retrieved', {'retrieval': {'mode': 'managed', 'uuid': receipt['descriptor']['ownership']['uuid']}, 'acquisition': {'folder': 'one'}})
        self.assertNotEqual(acquired['path'], receipt['descriptor']['path'])
        self.assertEqual((Path(acquired['path']) / 'tool.txt').read_text(), 'A')
        definition = {'registration_key': 'atomic', 'members': [
            {'name': 'good', 'target': {'type': 'acquire', 'source_uuid': source['uuid'], 'selector': request['selector'], 'selection': {'folder': 'one'}}},
            {'name': 'bad', 'target': {'type': 'acquire', 'source_uuid': source['uuid'], 'selector': request['selector'], 'selection': {'folder': 'missing'}}},
        ]}
        group = self.call('register_group', definition)
        try:
            self.call('prepare_group', group['uuid'])
        except Exception as error:
            self.assertIsNotNone(getattr(error, 'code', None))
        else:
            self.fail('missing member unexpectedly prepared')
        self.assertIsNone(self.call('show_group', group['uuid'])['selected'])
        (repo / 'one/tool.txt').write_text('B')
        git('add', '.')
        git('commit', '-m', 'B')
        newer = self.call('retrieve', request)
        self.assertNotEqual(newer['descriptor']['resolved']['commit'], receipt['descriptor']['resolved']['commit'])
        self.assertEqual((Path(acquired['path']) / 'tool.txt').read_text(), 'A')

    def tearDown(self):
        inventory = os.environ.get('SAUCEPAN_SDK_COVERAGE')
        if inventory:
            path = Path(inventory)
            prior = set(json.loads(path.read_text())) if path.exists() else set()
            path.write_text(json.dumps(sorted(prior | self.sdk.used)))
