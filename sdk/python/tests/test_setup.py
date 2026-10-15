import json
import unittest
from saucepan_sdk import Client, TransportError, SaucepanError, RunResult
from saucepan_sdk.transport import CliTransport, Call, Config, Payload

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
