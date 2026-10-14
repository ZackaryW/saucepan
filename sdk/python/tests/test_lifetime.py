import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import uuid
from saucepan_sdk import Client, RunOptions, TransportError

BINARY = str(Path(os.environ.get('SAUCEPAN_TEST_BINARY', Path(__file__).resolve().parents[3] / 'target/debug' / ('saucepan.exe' if os.name == 'nt' else 'saucepan'))).resolve())


class LifetimeTests(unittest.TestCase):
    def test_running_consumer_defers_reclaim_and_cancellation_preserves_pin(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            client = Client(executable=BINARY, root=str(base / 'root'))
            client.init()
            folder = base / 'source'
            folder.mkdir()
            (folder / 'file').write_text('A')
            source = client.register_source({'type': 'local', 'path': str(folder)})
            history, pin_policy = str(uuid.uuid4()), str(uuid.uuid4())
            client.register_policies([
                {'uuid': history, 'type': 'revision_policy', 'history': 'none'},
                {'uuid': pin_policy, 'type': 'pin_policy', 'retention': 'until_released'},
            ])
            request = {'source_uuid': source['uuid'], 'selector': {'type': 'local'}, 'policy_refs': [history]}
            a = client.acquire(request)
            ready, release = base / 'ready', base / 'release'
            child = [sys.executable, '-c', 'import pathlib,time,sys; pathlib.Path(sys.argv[1]).touch();\nwhile not pathlib.Path(sys.argv[2]).exists(): time.sleep(.02)', str(ready), str(release)]
            results = []
            thread = threading.Thread(target=lambda: results.append(client.run(a['entry']['uuid'], child, options=RunOptions(capture=True, timeout=10))))
            thread.start()
            try:
                deadline = time.monotonic() + 5
                while not ready.exists():
                    self.assertTrue(thread.is_alive())
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(.02)
                (folder / 'file').write_text('B')
                client.acquire(request)
                client.reclaim()
                self.assertTrue(Path(a['directory']).exists())
            finally:
                release.touch()
                thread.join(12)
            self.assertFalse(thread.is_alive())
            self.assertEqual(results[0].exit_code, 0)
            client.reclaim()
            self.assertFalse(Path(a['directory']).exists())
            b = client.acquire(request)
            pin = client.pin_outputs({'registration_key': 'retain', 'outputs': [b['entry']['uuid']], 'policy_refs': [pin_policy]})
            cancel = threading.Event()
            timer = threading.Timer(.2, cancel.set)
            timer.start()
            try:
                with self.assertRaises(TransportError) as error:
                    client.run(b['entry']['uuid'], [sys.executable, '-c', 'import time; time.sleep(.5)'], options=RunOptions(capture=True, cancel=cancel))
                self.assertEqual(error.exception.kind, 'cancelled')
                self.assertEqual(client.pin(pin['pin']['uuid'])['state'], 'active')
            finally:
                timer.join()
                # The child is intentionally independent of wrapper cancellation.
                time.sleep(.6)
            client.unpin(pin['pin']['uuid'])

    def test_public_identity_checks_precede_process_start(self):
        client = Client(executable='missing-sdk-executable').for_app('a')
        for operation in [lambda: client.claims('b'), lambda: client.query_claims({'app': 'b', 'where': []}),
                          lambda: client.acquire({'source_uuid': 's', 'selector': {'type': 'local'}}, app='b', scope='s')]:
            with self.assertRaises(TransportError) as error:
                operation()
            self.assertEqual(error.exception.kind, 'context_mismatch')
