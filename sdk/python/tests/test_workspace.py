import os
from pathlib import Path

import pytest

from saucepan_sdk import SaucepanError, Workspace


def test_default_binary_resolves_saucepan_through_path(
    saucepan_binary: Path,
    workspace: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("PATH", str(saucepan_binary.parent))

    client = Workspace(workspace)

    assert Path(client.binary).resolve() == saucepan_binary.resolve()


def test_explicit_binary_path_is_used_when_path_is_empty(
    saucepan_binary: Path,
    workspace: Path,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    empty_path = tmp_path / "empty-path"
    empty_path.mkdir()
    monkeypatch.setenv("PATH", str(empty_path))

    client = Workspace(workspace, binary=str(saucepan_binary))

    assert client.binary == str(saucepan_binary)


def test_missing_binary_failure_names_the_binary(
    workspace: Path, tmp_path: Path
) -> None:
    missing = tmp_path / ("missing-saucepan.exe" if os.name == "nt" else "missing-saucepan")

    with pytest.raises(SaucepanError) as caught:
        Workspace(workspace, binary=missing)

    assert str(missing) in str(caught.value)
    assert caught.value.exit_code is None
