"""Exercise the public Python client against the real central-store executable."""
import copy
import json
import subprocess
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import pytest

from saucepan_sdk import Saucepan, SaucepanError, shared_executable_path
from support import make_git_repository, update_repository


@pytest.fixture
def client(central_store):
    return Saucepan(binary=central_store.binary, test_root=central_store.root,
                    test_key=central_store.key)


def test_registration_settings_views_and_marker(client, central_store, tmp_path):
    token = client.register("app-a")
    original = copy.deepcopy(token)
    app = client.for_app(token)
    other = client.for_app(client.register("app-b"))
    source = tmp_path / "assets ü ; literal"
    source.mkdir()
    (source / "file.txt").write_text("hello", encoding="utf-8")
    recipe = {"source": {"provider": "local", "path": str(source)}}
    result = app.acquire(recipe)
    artifact = result["artifact"]
    assert Path(result["directory"], "file.txt").read_text() == "hello"
    assert app.path(artifact["id"]) == result["directory"]
    assert app.path("0" * 64) is None
    assert app.history("0" * 64) is None
    view = app.view()
    assert set(view["entries"]) == {artifact["id"]}
    assert other.view()["entries"] == {}
    assert app.verify(view) == {"verified": True}
    settings = {"retain_snapshots": True, "verify_content": True, "allow_local_fallback": False}
    assert app.configure(settings=settings, filters={"source_ids": [], "providers": ["git"]}) == {"configured": True}
    assert app.view()["entries"] == {}
    with pytest.raises(SaucepanError) as caught:
        app.verify(view)
    assert caught.value.exit_code == 1
    assert caught.value.stderr
    assert app.for_app("app-a").view()["settings"] == settings
    marker = tmp_path / ".saucepanhash"
    marker.write_text(json.dumps(token), encoding="utf-8")
    marked = Saucepan(binary=central_store.binary, marker=marker,
                     test_root=central_store.root, test_key=central_store.key)
    assert marked.view() == app.view()
    assert json.loads(marker.read_text()) == original == token
    app.configure(filters={"source_ids": [], "providers": []})
    assert app.acquire(recipe)["content_verified"] is True
    destination = tmp_path / "mirror ü"
    assert app.mirror(artifact["id"], destination) == {"directory": str(destination)}
    assert (destination / "file.txt").read_text() == "hello"
    with pytest.raises(SaucepanError):
        app.mirror(artifact["id"], destination)
    assert Path(client.shared_executable()) == shared_executable_path()


def test_init_and_failures(client, central_store, tmp_path):
    fresh = Saucepan(binary=central_store.binary, test_root=tmp_path / "new-store", test_key="31" * 32)
    assert fresh.init() == {"created": True}
    with pytest.raises(SaucepanError) as caught:
        fresh.init()
    assert caught.value.exit_code == 1
    with pytest.raises(SaucepanError):
        client.for_app("unknown").view()
    token = client.register("known")
    token["token"][0] ^= 1
    with pytest.raises(SaucepanError):
        client.for_app(token).view()
    with pytest.raises(SaucepanError):
        Saucepan(binary=central_store.binary, app="known", authoritative=True,
                 test_root=central_store.root, test_key=central_store.key).view()
    with pytest.raises(SaucepanError) as caught:
        client.register("")
    assert caught.value.exit_code in (1, 2)


def test_git_shared_folders_history_and_exact_reads(client, central_store):
    repo = make_git_repository(central_store.root.parent / "repo")
    (repo / "a").mkdir()
    (repo / "b").mkdir()
    (repo / "a/own.txt").write_text("own")
    (repo / "b/shared.txt").write_text("shared")
    subprocess.run(["git", "add", "a", "b"], cwd=repo, check=True, capture_output=True)
    blob = subprocess.run(["git", "hash-object", "-w", "--stdin"], input=b"../b/shared.txt",
                          cwd=repo, check=True, capture_output=True).stdout.decode().strip()
    subprocess.run(["git", "update-index", "--add", "--cacheinfo", f"120000,{blob},a/linked.txt"],
                   cwd=repo, check=True, capture_output=True)
    update_repository(repo, "1.0.0")
    app = client.for_app(client.register("git-app"))
    recipe = {"source": {"provider": "git", "origin": str(repo), "reference": "HEAD"}}
    first = app.acquire(recipe)
    assert not Path(first["directory"], ".git").exists()
    selected = app.acquire({**recipe, "folder": "a"})
    assert selected["artifact"]["snapshot_id"] == first["artifact"]["snapshot_id"]
    assert selected["artifact"]["source_id"] == first["artifact"]["source_id"]
    alias = Path(selected["directory"], "linked.txt")
    assert alias.read_text() == "shared" and not alias.is_symlink()
    update_repository(repo, "2.0.0")
    second = app.acquire(recipe)
    source = first["artifact"]["source_id"]
    snapshot = first["artifact"]["snapshot_id"]
    history = app.history(source)
    assert history["current"]["id"] == second["artifact"]["snapshot_id"]
    assert [entry["id"] for entry in history["history"]] == [snapshot]
    old = app.snapshot(source, snapshot)
    old_folder = app.snapshot(source, snapshot, folder="a")
    assert Path(old_folder["directory"], "linked.txt").read_text() == "shared"
    assert json.loads(Path(old["directory"], "sauce.json").read_text())["version"] == "1.0.0"
    pinned = app.acquire({**recipe, "commit": first["artifact"]["revision"]})
    assert pinned["artifact"]["snapshot_id"] == snapshot
    assert app.history(source)["current"]["id"] == second["artifact"]["snapshot_id"]


def test_concurrent_calls_and_literal_app_names(client):
    token = client.register("--literal app ; ü")
    app = client.for_app(token)
    with ThreadPoolExecutor(max_workers=4) as pool:
        views = list(pool.map(lambda _: app.view(), range(8)))
    assert all(view["app"] == token["app"] for view in views)
    assert client.for_app(token["app"]).view() == views[0]
