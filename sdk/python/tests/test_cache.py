import json
import subprocess
from pathlib import Path
from unittest import mock

from saucepan_sdk import Workspace

from support import make_git_repository


def test_two_sauce_reads_launch_cat_index_once(
    saucepan_binary: Path, workspace: Path
) -> None:
    client = Workspace(workspace, binary=saucepan_binary)

    with mock.patch(
        "saucepan_sdk._runner.subprocess.run", wraps=subprocess.run
    ) as run:
        assert client.sauces == ()
        assert client.sauces == ()

    cat_index_calls = [
        call for call in run.call_args_list if call.args[0][-2:] == ["cat", "index"]
    ]
    assert len(cat_index_calls) == 1


def test_install_invalidates_the_cached_index(
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
    assert client.sauces == ()

    client.install(repository.name)

    assert [sauce.name for sauce in client.sauces] == ["my-lib"]


def test_uninstall_invalidates_the_cached_index(
    saucepan_binary: Path, workspace: Path
) -> None:
    write_local_index(workspace)
    client = Workspace(workspace, binary=saucepan_binary)
    sauce = client.sauces[0]

    sauce.uninstall()

    assert client.sauces == ()


def test_bucket_add_and_remove_invalidate_the_cached_registry(
    saucepan_binary: Path, workspace: Path
) -> None:
    client = Workspace(workspace, binary=saucepan_binary)
    assert client.buckets == ()

    client.add_bucket("https://example.test/bucket.json")

    bucket = client.buckets[0]
    assert bucket.url == "https://example.test/bucket.json"

    bucket.remove()

    assert client.buckets == ()


def write_local_index(workspace: Path) -> None:
    state = workspace / ".saucepan"
    state.mkdir()
    (state / "index.json").write_text(
        json.dumps(
            [
                {
                    "source_type": "local",
                    "path": str(workspace / "local-source"),
                    "sauce": {
                        "name": "my-lib",
                        "version": "1.0.0",
                        "description": "Local sauce",
                    },
                }
            ]
        ),
        encoding="utf-8",
    )
