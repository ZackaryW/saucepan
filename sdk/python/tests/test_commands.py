import json
import os
import subprocess
from pathlib import Path

import pytest

from saucepan_sdk import Bucket, InternalError, Sauce, Workspace

from support import make_git_repository, update_repository


def configure_github(
    workspace: Path,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> Path:
    source_root = tmp_path / "sources"
    repository = make_git_repository(source_root / "repo.git")
    monkeypatch.setenv("GIT_CONFIG_COUNT", "1")
    monkeypatch.setenv(
        "GIT_CONFIG_KEY_0", "url.{}/.insteadOf".format(source_root.as_uri())
    )
    monkeypatch.setenv("GIT_CONFIG_VALUE_0", "https://github.com/owner/")
    (workspace / "saucepan.toml").write_text(
        '[github]\nbinary = "git"\n', encoding="utf-8"
    )
    return repository


def current_commit(repository: Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    return result.stdout.strip()


def write_bucket(path: Path) -> list:
    stubs = [
        {
            "name": "my-lib",
            "version": "1.0.0",
            "url": "https://example.test/my-lib/sauce.json",
        },
        {
            "name": "other-lib",
            "version": "2.0.0",
            "url": "https://example.test/other-lib/sauce.json",
        },
    ]
    path.write_text(json.dumps(stubs), encoding="utf-8")
    return stubs


def test_install_without_ref_returns_manifest_named_sauce(
    saucepan_binary: Path,
    workspace: Path,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    repository = configure_github(workspace, tmp_path, monkeypatch)
    client = Workspace(workspace, binary=saucepan_binary)

    sauce = client.install("owner/repo")

    assert isinstance(sauce, Sauce)
    assert sauce.name == "my-lib"
    assert sauce.reference is None
    assert sauce.resolved_commit == current_commit(repository)


def test_install_with_ref_records_requested_ref_and_resolved_commit(
    saucepan_binary: Path,
    workspace: Path,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    repository = configure_github(workspace, tmp_path, monkeypatch)
    subprocess.run(
        ["git", "tag", "v1"], cwd=repository, check=True, capture_output=True
    )
    client = Workspace(workspace, binary=saucepan_binary)

    sauce = client.install("owner/repo", reference="v1")

    assert sauce.name == "my-lib"
    assert sauce.reference == "v1"
    assert sauce.resolved_commit == current_commit(repository)


def test_update_and_uninstall_operate_on_the_sauce_entity(
    saucepan_binary: Path,
    workspace: Path,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    repository = configure_github(workspace, tmp_path, monkeypatch)
    client = Workspace(workspace, binary=saucepan_binary)
    sauce = client.install("owner/repo")
    update_repository(repository, "2.0.0")

    updated = sauce.update()

    assert updated is sauce
    assert sauce.version == "2.0.0"
    assert sauce.resolved_commit == current_commit(repository)

    sauce.uninstall()

    assert client.sauces == ()


def test_list_path_and_all_cat_targets_return_parsed_shapes(
    saucepan_binary: Path, workspace: Path, tmp_path: Path
) -> None:
    local_source = tmp_path / "local-source"
    local_source.mkdir()
    entry = {
        "source_type": "local",
        "path": str(local_source),
        "sauce": {
            "name": "my-lib",
            "version": "1.0.0",
            "description": "Local sauce",
        },
    }
    state = workspace / ".saucepan"
    state.mkdir()
    (state / "index.json").write_text(json.dumps([entry]), encoding="utf-8")
    bucket_file = tmp_path / "bucket.json"
    stubs = write_bucket(bucket_file)
    bucket_entry = {"url": str(bucket_file)}
    (state / "buckets.json").write_text(
        json.dumps([bucket_entry]), encoding="utf-8"
    )
    client = Workspace(workspace, binary=saucepan_binary)

    assert client.list() == [entry]
    assert client.path("my-lib") == local_source
    assert client.cat_index() == [entry]
    assert client.cat_buckets() == [bucket_entry]
    assert client.cat_sauce("my-lib") == entry
    assert client.cat_bucket(str(bucket_file)) == stubs


def test_bucket_add_remove_list_and_entity_operations(
    saucepan_binary: Path, workspace: Path, tmp_path: Path
) -> None:
    bucket_file = tmp_path / "bucket.json"
    stubs = write_bucket(bucket_file)
    client = Workspace(workspace, binary=saucepan_binary)

    bucket = client.add_bucket(str(bucket_file))

    assert isinstance(bucket, Bucket)
    assert client.list_buckets() == [{"url": str(bucket_file)}]
    assert bucket.stubs() == stubs

    bucket.remove()

    assert client.list_buckets() == []

    client.add_bucket(str(bucket_file))
    client.remove_bucket(str(bucket_file))

    assert client.list_buckets() == []


def test_search_returns_raw_parsed_values_for_field_predicate(
    saucepan_binary: Path, workspace: Path, tmp_path: Path
) -> None:
    bucket_file = tmp_path / "bucket.json"
    stubs = write_bucket(bucket_file)
    client = Workspace(workspace, binary=saucepan_binary)
    client.add_bucket(str(bucket_file))

    results = client.search('.name == "my-lib"')

    assert results == [stubs[0]]
    assert isinstance(results[0], dict)


def test_search_maps_successful_empty_messages_to_empty_results(
    saucepan_binary: Path, workspace: Path, tmp_path: Path
) -> None:
    client = Workspace(workspace, binary=saucepan_binary)
    assert client.search('.name == "my-lib"') == []

    bucket_file = tmp_path / "bucket.json"
    write_bucket(bucket_file)
    client.add_bucket(str(bucket_file))

    assert client.search('.name == "missing"') == []


def test_search_with_unavailable_jq_raises_internal_error(
    saucepan_binary: Path, workspace: Path, tmp_path: Path
) -> None:
    missing_jq = (tmp_path / "missing-jq.exe").as_posix()
    (workspace / "saucepan.toml").write_text(
        'jq = "{}"\n[local]\n'.format(missing_jq), encoding="utf-8"
    )
    bucket_file = tmp_path / "bucket.json"
    write_bucket(bucket_file)
    client = Workspace(workspace, binary=saucepan_binary)
    client.add_bucket(str(bucket_file))

    with pytest.raises(InternalError) as caught:
        client.search('.name == "my-lib"')

    assert caught.value.exit_code == 5
    assert "failed to spawn jq" in caught.value.stderr
