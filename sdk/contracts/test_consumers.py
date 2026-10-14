"""Isolated consumer-shaped evidence, not external application migration."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import uuid

from fixtures.resource_flow import PythonAdapter, BINARY
from test_node_resources import NodeAdapter


class ConsumerContracts(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='saucepan-consumer-contract-')
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.root = self.base / 'root'
        self.python = PythonAdapter(self.root)
        self.node = NodeAdapter(self.root)
        self.python.call('init')
        self.source_dir = self.base / 'source'
        self.source_dir.mkdir()
        for name in ['plugin', 'command', 'hook', 'service', 'skills']:
            (self.source_dir / name).mkdir()
            (self.source_dir / name / 'data.txt').write_text('A')
        self.source = self.python.call('register_source', {'type': 'local', 'path': str(self.source_dir)})['uuid']
        self.pin, self.history, self.update = [str(uuid.uuid4()) for _ in range(3)]
        self.python.call('register_policies', [
            {'uuid': self.pin, 'type': 'pin_policy', 'retention': 'until_released'},
            {'uuid': self.history, 'type': 'revision_policy', 'history': 'none'},
            {'uuid': self.update, 'type': 'update_policy', 'mode': 'always'},
        ])

    def group(self, key, names, *, scope=None):
        members = []
        for name in names:
            target = {'type': 'acquire', 'source_uuid': self.source, 'selector': {'type': 'local'},
                      'selection': {'folder': name}, 'policy_refs': [self.history, self.update]}
            if scope is not None:
                target['claim'] = {'scope': scope}
            members.append({'name': name, 'target': target})
        return {'registration_key': key, 'members': members}

    def test_power_eagle_pinned_handover_across_languages(self):
        group = self.python.call('register_group', self.group('plugins', ['plugin']))
        a = self.python.call('prepare_group', group['uuid'], expected_revision=group['revision'], pin={'registration_key': 'host-a', 'policy_refs': [self.pin]})
        old = a['resources']['members']['plugin']['directory']
        self.assertEqual(self.node.call('pin', a['pin']['uuid'])['uuid'], a['pin']['uuid'])
        (self.source_dir / 'plugin/data.txt').write_text('B')
        b = self.node.call('prepare_group', group['uuid'], expected_revision=a['group']['revision'], pin={'registration_key': 'host-b', 'policy_refs': [self.pin]})
        self.python.call('reclaim')
        # Both native updater processes have exited; an unrelated direct reader
        # accesses exact A while B is selected, without pretending to own a lease.
        result = subprocess.run(['node', '-e', 'console.log(require("fs").readFileSync(process.argv[1],"utf8"))', str(Path(old)/'data.txt')], capture_output=True, check=True)
        self.assertEqual(result.stdout.strip(), b'A')
        self.assertNotEqual(old, b['resources']['members']['plugin']['directory'])
        self.node.call('unpin', a['pin']['uuid'])
        self.python.call('reclaim')
        self.assertFalse(Path(old).exists())
        self.assertEqual(self.python.call('pin', b['pin']['uuid'])['state'], 'active')
        self.python.call('unpin', b['pin']['uuid'])

    def test_zush_named_command_hooks_service_and_cleanup(self):
        group = self.node.call('register_group', self.group('invocation', ['command', 'hook', 'service']))
        self.python.call('prepare_group', group['uuid'])
        cleanup = self.base/'cleaned'
        child = [sys.executable, '-c', 'import json,os,pathlib,sys; d=json.load(open(os.environ["SAUCEPAN_RESOURCE_GROUP_FILE"]));\ntry:\n assert set(d["members"])=={"command","hook","service"}\n for m in d["members"].values(): assert (pathlib.Path(m["directory"])/"data.txt").read_text()=="A"\nfinally: pathlib.Path(sys.argv[1]).touch()', str(cleanup)]
        run = self.node.call('run_registered_group', group['uuid'], child, options={'capture': True})
        self.assertEqual(run['exit_code'], 0, run['stderr'])
        self.assertTrue(cleanup.exists())

    def test_powerspec_lookup_and_explicit_project_upgrade(self):
        app = self.python.call('register_application', {'registration_key': 'powerspec-shaped', 'name': 'PowerSpec fixture'})['uuid']
        python, node = PythonAdapter(self.root, app), NodeAdapter(self.root, app)
        ga = python.call('register_group', self.group('project-a', ['skills'], scope='project-a'))
        gb = node.call('register_group', self.group('project-b', ['skills'], scope='project-b'))
        a = python.call('prepare_group', ga['uuid'])
        b = node.call('prepare_group', gb['uuid'])
        old = a['group']['selected']['members']['skills']['output_uuid']
        self.assertEqual(old, b['group']['selected']['members']['skills']['output_uuid'])
        self.assertNotEqual(a['group']['selected']['members']['skills']['claim_uuid'], b['group']['selected']['members']['skills']['claim_uuid'])
        (self.source_dir / 'skills/data.txt').write_text('B')
        query = node.call('query_claims', {'app': app, 'scope': 'project-a', 'where': []})
        self.assertEqual(query['claims'][0]['claim']['entry_uuid'], old)
        updated = node.call('prepare_group', ga['uuid'])
        self.assertNotEqual(updated['group']['selected']['members']['skills']['output_uuid'], old)
        still_b = python.call('query_claims', {'app': app, 'scope': 'project-b', 'where': []})
        self.assertEqual(still_b['claims'][0]['claim']['entry_uuid'], old)
