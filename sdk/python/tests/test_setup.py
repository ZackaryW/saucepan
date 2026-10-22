import json
import unittest
from saucepan_sdk import Client, TransportError, SaucepanError, RunResult
from saucepan_sdk.transport import CliTransport, Call, Config, Payload
import uuid

def setup_receipt(status='complete'):
    return dict(schema_version=1, root='root', status=status, changed=status == 'complete',
                launcher_path='root/bin/saucepan', app=str(uuid.uuid4()), claim=str(uuid.uuid4()),
                output=str(uuid.uuid4()), installed_version='v-next', operation=str(uuid.uuid4()),
                lifecycle_operation=str(uuid.uuid4()), backup_paths=[], diagnostics=[], retry_shim=None)

def receipt(status):
    return dict(operation='operation', app='app', claim='claim', fingerprint='hash', status=status,
                changed=False, action={'type': 'update'}, pending=[], leftovers=[], shims=[],
                candidate=None, desired=None, desired_registry=None, source_policy=None, reason=None, expected_revision=None)

class Harness(CliTransport):
    def __init__(self, value, code, stderr=b''):
        self.value, self.code, self.stderr = value, code, stderr
    def _invoke(self, config, call, options):
        return RunResult(self.code, None, json.dumps(self.value).encode(), self.stderr)

class SetupTests(unittest.TestCase):
    def test_setup_receipt_validation_and_status_observation(self):
        for phase in ['complete', 'preparing', 'commit_intent', 'cleanup_pending', 'failed', 'conflict']:
            value = setup_receipt(phase)
            code = 0 if phase == 'complete' else 2
            result = Harness(value, code).json(Config(), Call(['setup'], mutates=True, lifecycle='setup'))
            self.assertEqual(result, {'exit_code': code, 'result': value})
        good = setup_receipt()
        for value, code in [(good, 2), ({**good, 'app': None}, 0), ({**good, 'schema_version': True}, 0),
                            ({**good, 'operation': 'not-a-uuid'}, 0), ({**good, 'retry_shim': 3}, 0)]:
            with self.assertRaises(TransportError) as error:
                Harness(value, code).json(Config(), Call(['setup'], mutates=True, lifecycle='setup'))
            self.assertTrue(error.exception.indeterminate)
        observation = dict(schema_version=1, root='missing', state='absent', launcher_path=None,
                           app=None, claim=None, output=None, installed_version=None, operation=None, diagnostics=[])
        self.assertEqual(Harness(observation, 0).json(Config(), Call(['setup-status'], lifecycle='setup_status')), observation)
        with self.assertRaises(TransportError) as error:
            Harness({**observation, 'state': 'ready'}, 0).json(Config(), Call(['setup-status'], lifecycle='setup_status'))
        self.assertFalse(error.exception.indeterminate)

    def test_setup_maps_literals_and_rejects_bound_client_without_execution(self):
        calls = []
        class Spy:
            def json(self, config, call):
                calls.append((config, call)); return {}
        client = Client(root='root space 雪', transport=Spy())
        client.setup('shim space 雪', tag='v-next', operation='op', migrate_unmanaged=True, expected_sha256='hash')
        config, call = calls[-1]
        self.assertEqual(config.root, 'root space 雪')
        self.assertEqual(call.argv, ['setup', 'shim space 雪', '--tag', 'v-next', '--operation', 'op', '--expected-sha256', 'hash', '--migrate-unmanaged'])
        self.assertEqual(call.lifecycle, 'setup')
        client.setup_status()
        self.assertEqual(calls[-1][1].argv, ['setup-status'])
        client.self_update('app', 'claim', tag='v-next')
        self.assertEqual(calls[-1][1].argv, ['self-update', 'app', 'claim', '--tag', 'v-next'])
        count = len(calls)
        for action in [lambda: client.for_app('app').setup('shim', tag='v-next'), lambda: client.for_app('app').setup_status()]:
            with self.assertRaises(TransportError): action()
        self.assertEqual(len(calls), count)

    def test_receipts_and_mixed_reconcile_are_explicit(self):
        for status in ['complete','preparing','commit_intent','cleanup_pending','failed','conflict']:
            code = 0 if status == 'complete' else 2
            result = Harness(receipt(status), code).json(Config(), Call(['update-app'], mutates=True, lifecycle='operation'))
            self.assertEqual(result, {'exit_code': code, 'result': receipt(status)})
        mixed = [receipt('complete'), receipt('failed')]
        self.assertEqual(Harness(mixed, 2).json(Config(), Call(['reconcile'], lifecycle='reconcile'))['result'], mixed)
        self.assertEqual(Harness(mixed, 0).json(Config(), Call(['operations'], lifecycle='inspection')), mixed)
        self.assertEqual(Harness([], 0).json(Config(), Call(['reconcile'], lifecycle='reconcile'))['result'], [])

    def test_malformed_and_inconsistent_receipts_keep_uncertainty(self):
        for value, code in [(receipt('complete'), 2), (receipt('failed'), 0), ({}, 2), ([receipt('failed')], 2),
                            ({**receipt('complete'), 'changed': 'yes'}, 0), ({**receipt('failed'), 'candidate': []}, 2)]:
            with self.subTest(value=value), self.assertRaises(TransportError) as error:
                Harness(value, code).json(Config(), Call(['uninstall'], mutates=True, lifecycle='operation'))
            self.assertTrue(error.exception.indeterminate)
        with self.assertRaises(SaucepanError) as error:
            Harness(None, 1, b'{"error":{"code":"denied","message":"denied","details":{}}}').json(Config(), Call(['uninstall'], lifecycle='operation'))
        self.assertEqual(error.exception.code, 'denied')
        with self.assertRaises(TransportError):
            Harness(receipt('failed'), 2).json(Config(), Call(['query-sources']))

    def test_literal_mapping_omission_identity_and_no_automatic_calls(self):
        calls = []
        class Spy:
            def json(self, config, call):
                calls.append((config, call)); return {}
        client = Client(executable='missing', transport=Spy())
        self.assertEqual(calls, [])
        client.profile()
        self.assertEqual(calls[-1][1].argv, ['profile'])
        client.profile('managed', for_install=True, catalog=[], retrieval_policy_refs=['r'], acquisition_policy_refs=['a'])
        self.assertEqual(calls[-1][1].argv[:4], ['profile','managed','--for-install','--catalog'])
        self.assertIsInstance(calls[-1][1].argv[4], Payload)
        client.install_application('name;雪', {}, {}, 'shim space', scope='project', shim='tool', operation='op')
        call = calls[-1][1]
        self.assertEqual(call.argv[:2], ['install', 'name;雪'])
        self.assertEqual(call.argv[4:], ['shim space','--scope','project','--shim','tool','--operation','op'])
        self.assertEqual(call.lifecycle, 'operation')
        bound = client.for_app('a')
        bound.update_application('a', 'claim', {})
        self.assertEqual(calls[-1][0].app, 'a')
        before = len(calls)
        for invoke in [lambda: bound.self_update('b','c'), lambda: bound.operations(app='b'),
                       lambda: bound.explain_policies({'operation':'claim','app':'b','claim':'c'})]:
            with self.assertRaises(TransportError): invoke()
        self.assertEqual(len(calls), before)
        with self.assertRaises(TransportError) as error:
            Client(executable='missing-saucepan-sdk-setup').bootstrap({}, 'shim')
        self.assertEqual(error.exception.kind, 'missing_executable')
        self.assertFalse(error.exception.indeterminate)
