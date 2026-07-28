import json
import os
import subprocess
from pathlib import Path

import pytest

from saucepan_sdk import (
    ConfigError,
    Conflict,
    InternalError,
    NotFound,
    SaucepanError,
    SourceError,
)
from saucepan_sdk._runner import CommandRunner


def test_zero_exit_returns_parsed_json(
    saucepan_binary: Path, workspace: Path
) -> None:
    runner = CommandRunner(saucepan_binary, workspace)

    assert runner.run_json("cat", "index") == []


def test_exit_one_raises_not_found_with_diagnostics(
    saucepan_binary: Path, workspace: Path
) -> None:
    runner = CommandRunner(saucepan_binary, workspace)

    with pytest.raises(NotFound) as caught:
        runner.run_json("path", "missing")

    assert_error(caught.value, 1, "missing")


def test_exit_two_raises_source_error_with_diagnostics(
    saucepan_binary: Path,
    workspace: Path,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    (workspace / "saucepan.toml").write_text(
        '[github]\nbinary = "gh"\n', encoding="utf-8"
    )
    empty_path = tmp_path / "empty-path"
    empty_path.mkdir()
    monkeypatch.setenv("PATH", str(empty_path))
    runner = CommandRunner(saucepan_binary, workspace)

    with pytest.raises(SourceError) as caught:
        runner.run_json("install", "owner/repo")

    assert_error(caught.value, 2, "owner/repo")


def test_exit_three_raises_config_error_with_diagnostics(
    saucepan_binary: Path, tmp_path: Path
) -> None:
    invalid_workspace = tmp_path / "missing-config"
    invalid_workspace.mkdir()
    runner = CommandRunner(saucepan_binary, invalid_workspace)

    with pytest.raises(ConfigError) as caught:
        runner.run_json("cat", "index")

    assert_error(caught.value, 3, "saucepan.toml")


def test_exit_four_raises_conflict_with_diagnostics(
    saucepan_binary: Path, workspace: Path, tmp_path: Path
) -> None:
    if not shutil_which("git"):
        pytest.skip("git is required for the real source-conflict fixture")
    repository = make_git_repository(tmp_path / "repository")
    (workspace / "saucepan.toml").write_text(
        '[customgit]\nurl = "{}"\nbinary = "git"\n'.format(
            tmp_path.as_posix()
        ),
        encoding="utf-8",
    )
    index_dir = workspace / ".saucepan"
    index_dir.mkdir()
    (index_dir / "index.json").write_text(
        json.dumps(
            [
                {
                    "source_type": "local",
                    "path": str(tmp_path / "local"),
                    "sauce": {
                        "name": "my-lib",
                        "version": "1.0.0",
                        "description": "local",
                    },
                }
            ]
        ),
        encoding="utf-8",
    )
    runner = CommandRunner(saucepan_binary, workspace)

    with pytest.raises(Conflict) as caught:
        runner.run_json("install", repository.name)

    assert_error(caught.value, 4, "different source type")


def test_exit_five_raises_internal_error_with_diagnostics(
    saucepan_binary: Path, workspace: Path
) -> None:
    index_dir = workspace / ".saucepan"
    index_dir.mkdir()
    (index_dir / "index.json").write_text("not json", encoding="utf-8")
    runner = CommandRunner(saucepan_binary, workspace)

    with pytest.raises(InternalError) as caught:
        runner.run_json("cat", "index")

    assert_error(caught.value, 5, "invalid index.json")


def assert_error(error: SaucepanError, code: int, stderr_fragment: str) -> None:
    assert error.exit_code == code
    assert stderr_fragment in error.stderr
    assert isinstance(error, SaucepanError)


def shutil_which(command: str) -> str:
    result = subprocess.run(
        ["where" if os.name == "nt" else "which", command],
        check=False,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip() if result.returncode == 0 else ""


def make_git_repository(path: Path) -> Path:
    path.mkdir()
    (path / "sauce.json").write_text(
        json.dumps(
            {
                "name": "my-lib",
                "version": "2.0.0",
                "description": "remote",
            }
        ),
        encoding="utf-8",
    )
    environment = {
        **os.environ,
        "GIT_AUTHOR_NAME": "test",
        "GIT_AUTHOR_EMAIL": "test@example.com",
        "GIT_COMMITTER_NAME": "test",
        "GIT_COMMITTER_EMAIL": "test@example.com",
    }
    for args in (("init",), ("add", "sauce.json"), ("commit", "-m", "initial")):
        subprocess.run(
            ["git", *args], cwd=path, env=environment, check=True, capture_output=True
        )
    return path
