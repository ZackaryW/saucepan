"""Process-boundary failures use real child processes; no CLI behavior is mocked."""
import os
import sys
from pathlib import Path

import pytest

from saucepan_sdk import Saucepan, SaucepanError, shared_executable_path
from saucepan_sdk._runner import execute


def test_default_binary_and_invalid_contexts(tmp_path):
    assert Saucepan().binary == str(shared_executable_path())
    for options in ({"test_root": tmp_path}, {"test_key": "12" * 32},
                    {"test_root": tmp_path, "test_key": "secret"},
                    {"marker": "marker", "token": {}}, {"timeout": 0},
                    {"timeout": float("nan")}, {"timeout": -1}):
        with pytest.raises(ValueError):
            Saucepan(**options)
    with pytest.raises(SaucepanError) as caught:
        Saucepan(binary=tmp_path / "missing", test_root=tmp_path, test_key="ab" * 32).view()
    assert caught.value.exit_code is None
    assert "ab" * 32 not in str(caught.value)


def test_process_json_exit_and_timeout():
    assert execute(sys.executable, ["-c", "print('null')"], None) is None
    with pytest.raises(SaucepanError) as caught:
        execute(sys.executable, ["-c", "import sys; print('partial'); print('diagnostic', file=sys.stderr); sys.exit(2)"], None)
    assert caught.value.exit_code == 2
    assert caught.value.stdout.strip() == "partial"
    assert caught.value.stderr.strip() == "diagnostic"
    with pytest.raises(SaucepanError) as caught:
        execute(sys.executable, ["-c", "print('not-json')"], None)
    assert caught.value.code == "INVALID_JSON"
    assert caught.value.exit_code == 0
    with pytest.raises(SaucepanError) as caught:
        execute(sys.executable, ["-c", "import time; time.sleep(60) # private-test-secret"], 0.1)
    assert caught.value.code == "TIMEOUT"
    assert "private-test-secret" not in str(caught.value)
    assert caught.value.exit_code is None


@pytest.mark.parametrize("value", [{"version": 2}, {"version": True}, {}, [], None])
def test_unknown_response_versions_fail(value):
    with pytest.raises(SaucepanError) as caught:
        Saucepan._versioned(value)
    assert caught.value.code == "PROTOCOL_VERSION"


def test_request_cleanup_on_success_and_failure(central_store, tmp_path, monkeypatch):
    import saucepan_sdk.client as module
    original = module.execute
    paths = []

    def checked_execute(binary, arguments, timeout):
        for argument in arguments:
            if argument.startswith("--marker="):
                marker = Path(argument.split("=", 1)[1])
                paths.append(marker)
                assert marker.exists()
                if os.name != "nt":
                    assert marker.stat().st_mode & 0o777 == 0o600
        return original(binary, arguments, timeout)

    monkeypatch.setattr(module, "execute", checked_execute)
    store = Saucepan(binary=central_store.binary, test_root=central_store.root, test_key=central_store.key)
    token = store.register("cleanup")
    app = store.for_app(token)
    token["token"][0] ^= 1  # Client owns its caller context, independent of the input.
    app.view()
    with pytest.raises(SaucepanError):
        app.acquire({"source": {"provider": "local", "path": str(tmp_path / "absent")}})
    assert len(paths) == 2 and paths[0].parent != paths[1].parent
    assert all(not path.parent.exists() for path in paths)
