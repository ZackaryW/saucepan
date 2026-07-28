import json
import subprocess
from pathlib import Path
from unittest import mock

from saucepan_sdk import Workspace

from support import make_git_repository, update_repository


def test_update_refreshes_the_same_sauce_object(
    saucepan_binary: Path, workspace: Path, tmp_path: Path
) -> None:
    repository = make_git_repository(tmp_path / "repository")
    (workspace / "saucepan.toml").write_text(
        '[customgit]\nurl = "{}"\nbinary = "git"\n'.format(
            tmp_path.as_posix()
        ),
        encoding="utf-8",
    )
    client = Workspace(workspace, binary=saucepan_binary)
    client.install(repository.name)
    sauce = client.sauces[0]
    assert sauce.version == "1.0.0"
    update_repository(repository, "2.0.0")

    result = sauce.update()

    assert result is sauce
    assert sauce.version == "2.0.0"
    assert sauce.description == "Updated sauce"


def test_workspace_refresh_eagerly_rereads_the_full_index(
    saucepan_binary: Path, workspace: Path
) -> None:
    write_index(workspace, "1.0.0")
    client = Workspace(workspace, binary=saucepan_binary)
    assert client.sauces[0].version == "1.0.0"
    write_index(workspace, "2.0.0")

    with mock.patch(
        "saucepan_sdk._runner.subprocess.run", wraps=subprocess.run
    ) as run:
        client.refresh()
        assert client.sauces[0].version == "2.0.0"

    cat_index_calls = [
        call for call in run.call_args_list if call.args[0][-2:] == ["cat", "index"]
    ]
    assert len(cat_index_calls) == 1


def write_index(workspace: Path, version: str) -> None:
    state = workspace / ".saucepan"
    state.mkdir(exist_ok=True)
    (state / "index.json").write_text(
        json.dumps(
            [
                {
                    "source_type": "local",
                    "path": str(workspace / "local-source"),
                    "sauce": {
                        "name": "my-lib",
                        "version": version,
                        "description": "Local sauce",
                    },
                }
            ]
        ),
        encoding="utf-8",
    )
