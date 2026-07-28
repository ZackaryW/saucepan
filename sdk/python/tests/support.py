import json
import os
import subprocess
from pathlib import Path


def make_git_repository(
    path: Path, version: str = "1.0.0", description: str = "Test sauce"
) -> Path:
    path.mkdir(parents=True)
    write_manifest(path, version, description)
    subprocess.run(["git", "init"], cwd=path, check=True, capture_output=True)
    commit_all(path, "initial")
    return path


def update_repository(path: Path, version: str) -> None:
    write_manifest(path, version, "Updated sauce")
    commit_all(path, "update manifest")


def write_manifest(path: Path, version: str, description: str) -> None:
    (path / "sauce.json").write_text(
        json.dumps(
            {
                "name": "my-lib",
                "version": version,
                "description": description,
            }
        ),
        encoding="utf-8",
    )


def commit_all(path: Path, message: str) -> None:
    environment = {
        **os.environ,
        "GIT_AUTHOR_NAME": "test",
        "GIT_AUTHOR_EMAIL": "test@example.com",
        "GIT_COMMITTER_NAME": "test",
        "GIT_COMMITTER_EMAIL": "test@example.com",
    }
    subprocess.run(
        ["git", "add", "sauce.json"],
        cwd=path,
        env=environment,
        check=True,
        capture_output=True,
    )
    subprocess.run(
        ["git", "commit", "-m", message],
        cwd=path,
        env=environment,
        check=True,
        capture_output=True,
    )
