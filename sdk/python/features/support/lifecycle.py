"""Shared lifecycle helpers for SDK-owned Behave capabilities."""

import json
import os
import subprocess
import tempfile
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[4]


def before_all(context) -> None:
    subprocess.run(
        ["cargo", "build", "--bin", "saucepan"],
        cwd=REPOSITORY_ROOT,
        check=True,
    )
    executable = "saucepan.exe" if os.name == "nt" else "saucepan"
    context.saucepan_binary = REPOSITORY_ROOT / "target" / "debug" / executable


def before_scenario(context, _scenario) -> None:
    context.temporary_directory = tempfile.TemporaryDirectory()
    context.test_root = Path(context.temporary_directory.name)
    context.workspace_root = context.test_root / "workspace"
    context.workspace_root.mkdir()
    (context.workspace_root / "saucepan.toml").write_text(
        '[local]\n[index]\nbinary = "git"\n', encoding="utf-8"
    )


def after_scenario(context, _scenario) -> None:
    context.temporary_directory.cleanup()


def _commit(repository: Path, message: str) -> str:
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
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    return result.stdout.strip()


def make_index_repository(context, tag: str) -> Path:
    repository = context.test_root / "index-repository"
    repository.mkdir()
    (repository / "bucket.json").write_text("[]", encoding="utf-8")
    subprocess.run(["git", "init"], cwd=repository, check=True, capture_output=True)
    context.index_commit = _commit(repository, "initial index")
    subprocess.run(["git", "tag", tag], cwd=repository, check=True, capture_output=True)
    return repository


def advance_index(repository: Path) -> str:
    entries = json.loads((repository / "bucket.json").read_text(encoding="utf-8"))
    entries.append(
        {
            "name": "advanced",
            "version": "2.0.0",
            "url": "https://example.test/advanced",
        }
    )
    (repository / "bucket.json").write_text(json.dumps(entries), encoding="utf-8")
    return _commit(repository, "advance index")


def write_sdk_state_fixture(context) -> None:
    state = context.workspace_root / ".saucepan"
    state.mkdir(exist_ok=True)
    bucket_file = context.test_root / "bucket.json"
    bucket_file.write_text(
        json.dumps(
            [
                {
                    "name": "my-lib",
                    "version": "1.0.0",
                    "url": "https://example.test/my-lib",
                    "commands": {"run": "bin/run"},
                }
            ]
        ),
        encoding="utf-8",
    )
    (state / "buckets.json").write_text(
        json.dumps([{"url": str(bucket_file)}]), encoding="utf-8"
    )
    (state / "index.json").write_text(
        json.dumps(
            [
                {
                    "source_type": "github",
                    "repo": "owner/repo",
                    "manifest_source": {"kind": "index", "index": str(bucket_file)},
                    "sauce": {
                        "name": "my-lib",
                        "version": "1.0.0",
                        "description": "Indexed sauce",
                    },
                }
            ]
        ),
        encoding="utf-8",
    )
