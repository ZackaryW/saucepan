import concurrent.futures
from contextlib import contextmanager
import json
from pathlib import Path
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

from saucepan_sdk import Client, SaucepanError, TransportError
from saucepan_sdk.transport import Call, CliTransport, Config, Payload
from saucepan_sdk.utils.payloads import decode, payload_files
from saucepan_sdk.utils.process import invoke


class ProcessTests(unittest.TestCase):
    def test_literal_unicode_arguments_and_both_streams(self):
        args = ['a b', '雪', '"quoted"', '$(echo bad); & | >']
        result = invoke(sys.executable, ['-c', 'import json,sys; print(json.dumps(sys.argv[1:])); sys.stderr.write("e"*100000)', *args])
        self.assertEqual(json.loads(result.stdout), args)
        self.assertEqual(len(result.stderr), 100000)

    def test_limit_deadline_missing_and_cancellation(self):
        cases = [(['-c', 'import sys; sys.stdout.write("x"*100000)'], dict(capture_limit=16), 'output_limit'),
                 (['-c', 'import time; time.sleep(20)'], dict(timeout=0.1), 'timeout')]
        for argv, options, kind in cases:
            with self.subTest(kind=kind), self.assertRaises(TransportError) as error:
                invoke(sys.executable, argv, **options)
            self.assertEqual(error.exception.kind, kind)
            self.assertTrue(error.exception.started)
        with self.assertRaises(TransportError) as error:
            invoke('missing-saucepan-sdk-test-executable', [])
        self.assertEqual(error.exception.kind, 'missing_executable')
        event = threading.Event()
        timer = threading.Timer(0.1, event.set)
        timer.start()
        try:
            with self.assertRaises(TransportError) as error:
                invoke(sys.executable, ['-c', 'import time; time.sleep(20)'], cancel=event)
            self.assertEqual(error.exception.kind, 'cancelled')
        finally:
            timer.join()

    def test_raw_nonzero_and_invalid_utf8(self):
        result = invoke(sys.executable, ['-c', 'import sys; sys.stdout.buffer.write(bytes([255])); sys.exit(7)'])
        self.assertEqual(result.exit_code, 7)
        self.assertEqual(result.stdout, b'\xff')
        with self.assertRaises(TransportError):
            decode(result.stdout)


class PayloadTests(unittest.TestCase):
    def test_owned_closed_isolated_files_and_cleanup(self):
        barrier = threading.Barrier(2)
        value = {'unicode': '雪', 'empty': [], 'explicit': None}
        def worker(_):
            with payload_files({'request': value}, limit=1024) as paths:
                path = Path(paths['request'])
                self.assertEqual(json.loads(path.read_bytes()), value)
                barrier.wait(timeout=3)
                return path
        with concurrent.futures.ThreadPoolExecutor(2) as pool:
            paths = list(pool.map(worker, range(2)))
        self.assertNotEqual(paths[0].parent, paths[1].parent)
        self.assertTrue(all(not path.parent.exists() for path in paths))
        self.assertEqual(value, {'unicode': '雪', 'empty': [], 'explicit': None})

    def test_failure_cleanup_and_input_validation(self):
        with self.assertRaises(RuntimeError):
            with payload_files({'x': {}}, limit=1024) as paths:
                directory = Path(paths['x']).parent
                raise RuntimeError()
        self.assertFalse(directory.exists())
        for value in [2**53, float('nan'), {1: 'x'}, object(), {'x': 'a'*2000}]:
            with self.subTest(value=type(value)), self.assertRaises(TransportError):
                with payload_files({'x': value}, limit=1024):
                    self.fail('invalid input admitted')


class TransportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.script = Path(self.temp.name) / 'fake.py'

    def execute(self, script, *, mutates=False, payload=None, required=()):
        self.script.write_text(script, encoding='utf-8')
        class ProcessHarness(CliTransport):
            def _invoke(inner, config, call, options):
                return invoke(sys.executable, [str(self.script)], capture=True)
        return ProcessHarness().json(Config(), Call([], mutates=mutates, required=required))

    def test_inert_constructor_and_context(self):
        client = Client(executable='absent-test-binary')
        bound = client.for_app('a', credential_file='not-read')
        with self.assertRaises(TransportError) as error:
            bound._identity('b')
        self.assertEqual(error.exception.kind, 'context_mismatch')

    def test_json_and_domain_error(self):
        self.assertEqual(self.execute('print(\'{"uuid":"a"}\')', required=('uuid',)), {'uuid': 'a'})
        with self.assertRaises(SaucepanError) as error:
            self.execute('import sys; sys.stderr.write(\'{"error":{"code":"revision_conflict","message":"conflict","details":{"member":"a"}}}\'); sys.exit(1)')
        self.assertEqual(error.exception.code, 'revision_conflict')
        self.assertEqual(error.exception.details, {'member': 'a'})

    def test_protocol_uncertainty(self):
        for source in ['print("no json")', 'print("[]")', 'print("{}")', 'print(\'{"uuid":9007199254740992}\')']:
            with self.subTest(source=source), self.assertRaises(TransportError) as error:
                self.execute(source, mutates=True, required=('uuid',))
            self.assertEqual(error.exception.kind, 'protocol')
            self.assertTrue(error.exception.indeterminate)

    def test_cleanup_failure_preserves_mutation_uncertainty(self):
        marker = Path(self.temp.name) / 'committed'

        @contextmanager
        def failing_cleanup(values, *, limit):
            with payload_files(values, limit=limit) as paths:
                yield paths
            raise OSError('simulated cleanup failure')

        def execute_mutation(*args, **kwargs):
            return invoke(sys.executable, ['-c',
                'from pathlib import Path; import sys; Path(sys.argv[1]).touch(); print("{}")',
                str(marker)])

        with patch('saucepan_sdk.transport.payload_files', failing_cleanup), \
             patch('saucepan_sdk.transport.invoke', execute_mutation), \
             self.assertRaises(TransportError) as error:
            CliTransport().json(Config(), Call([], mutates=True))
        self.assertTrue(marker.exists())
        self.assertEqual(error.exception.kind, 'payload_io')
        self.assertTrue(error.exception.started)
        self.assertTrue(error.exception.indeterminate)

        with patch('saucepan_sdk.transport.payload_files', side_effect=OSError()), \
             self.assertRaises(TransportError) as error:
            CliTransport().json(Config(), Call([], mutates=True))
        self.assertFalse(error.exception.started)
        self.assertFalse(error.exception.indeterminate)


if __name__ == '__main__':
    unittest.main()
