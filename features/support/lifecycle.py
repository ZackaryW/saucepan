"""Shared lifecycle and public-process helpers for root Behave capabilities."""

import json
import os
import subprocess
import tempfile
from pathlib import Path
from typing import Mapping, Optional, Sequence


_GIT_ENVIRONMENT_KEYS = (
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_KEY_0",
    "GIT_CONFIG_VALUE_0",
)


def repository_root() -> Path:
    return Path(__file__).resolve().parents[2]


def built_binary() -> Path:
    executable = "saucepan.exe" if os.name == "nt" else "saucepan"
    return repository_root() / "target" / "debug" / executable


def before_all(context) -> None:
    subprocess.run(
        ["cargo", "build", "--bin", "saucepan"],
        cwd=repository_root(),
        check=True,
    )
    context.saucepan_binary = built_binary()


def before_scenario(context, _scenario) -> None:
    context.temporary_directory = tempfile.TemporaryDirectory()
    context.test_root = Path(context.temporary_directory.name)
    context.workspace_root = context.test_root / "workspace"
    context.workspace_root.mkdir()
    context.saved_git_environment = {
        key: os.environ.get(key) for key in _GIT_ENVIRONMENT_KEYS
    }
    context.command_results = []


def after_scenario(context, _scenario) -> None:
    for key, value in context.saved_git_environment.items():
        if value is None:
            os.environ.pop(key, None)
        else:
            os.environ[key] = value
    context.temporary_directory.cleanup()


def write_workspace_config(context, contents: str) -> None:
    (context.workspace_root / "saucepan.toml").write_text(contents, encoding="utf-8")


def run_cli(
    context,
    *args: str,
    workspace: Optional[Path] = None,
) -> subprocess.CompletedProcess:
    result = subprocess.run(
        [str(context.saucepan_binary), str(workspace or context.workspace_root), *args],
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    context.result = result
    context.command_results.append(result)
    return result


def make_git_repository(
    context,
    name: str,
    manifest: Optional[Mapping[str, object]] = None,
) -> Path:
    repository = context.test_root / name
    repository.mkdir(parents=True)
    if manifest is None:
        (repository / "README.md").write_text("fixture\n", encoding="utf-8")
    else:
        (repository / "sauce.json").write_text(
            json.dumps(dict(manifest)), encoding="utf-8"
        )
    subprocess.run(["git", "init"], cwd=repository, check=True, capture_output=True)
    commit_repository(repository, "initial")
    return repository


def commit_repository(repository: Path, message: str) -> str:
    environment = {
        **os.environ,
        "GIT_AUTHOR_NAME": "test",
        "GIT_AUTHOR_EMAIL": "test@example.com",
        "GIT_COMMITTER_NAME": "test",
        "GIT_COMMITTER_EMAIL": "test@example.com",
    }
    subprocess.run(
        ["git", "add", "."],
        cwd=repository,
        env=environment,
        check=True,
        capture_output=True,
    )
    subprocess.run(
        ["git", "commit", "-m", message],
        cwd=repository,
        env=environment,
        check=True,
        capture_output=True,
    )
    return repository_commit(repository)


def tag_repository(repository: Path, tag: str) -> str:
    subprocess.run(["git", "tag", tag], cwd=repository, check=True, capture_output=True)
    return repository_commit(repository)


def repository_commit(repository: Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    return result.stdout.strip()


def read_workspace_json(context, relative_path: str):
    return json.loads((context.workspace_root / relative_path).read_text(encoding="utf-8"))


def write_bucket(path: Path, entries: Sequence[Mapping[str, object]]) -> None:
    path.write_text(json.dumps([dict(entry) for entry in entries]), encoding="utf-8")
