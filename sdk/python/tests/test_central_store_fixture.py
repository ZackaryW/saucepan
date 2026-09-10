"""Real CLI interoperability fixtures, not legacy Workspace API compatibility."""

import json
from pathlib import Path

from central_store_support import CentralTestStore
from support import make_git_repository


def test_isolated_key_reopens_only_matching_store(central_store: CentralTestStore) -> None:
    store = central_store
    store.json("register", "fixture")
    assert store.json("--app", "fixture", "view")["entries"] == {}
    key = store.key
    index = (store.root / "index.json.enc").read_bytes()
    store.key = bytes(byte ^ 1 for byte in bytes.fromhex(key)).hex()
    wrong = store.run("--app", "fixture", "view")
    assert wrong.returncode == 1
    assert wrong.stdout == b""
    assert (store.root / "index.json.enc").read_bytes() == index
    store.key = key
    assert store.run("--app", "fixture", "view").returncode == 0
    assert not (store.root / ".saucepan").exists()
    assert not (store.root / "key.bin").exists()


def test_real_recipe_acquisition_uses_scoped_test_store(central_store, tmp_path) -> None:
    store = central_store
    repository = make_git_repository(tmp_path / "repository")
    proof = store.json("register", "app-a")
    store.json("register", "app-b")
    marker = tmp_path / ".saucepanhash"
    marker.write_text(json.dumps(proof), encoding="utf-8")
    recipe = tmp_path / "recipe.json"
    recipe.write_text(json.dumps({
        "source": {"provider": "git", "origin": str(repository), "reference": "HEAD"},
    }), encoding="utf-8")
    assert store.run("--app", "app-a", "--authoritative", "acquire", str(recipe)).returncode == 1
    artifact = store.json("--marker", str(marker), "acquire", str(recipe))
    directory = Path(artifact["directory"])
    assert json.loads((directory / "sauce.json").read_text())["name"] == "my-lib"
    assert not (directory / ".git").exists()
    view = store.json("--marker", str(marker), "view")
    assert set(view["entries"]) == {artifact["artifact"]["id"]}
    assert store.json("--app", "app-b", "view")["entries"] == {}
    saved_view = tmp_path / "view.json"
    saved_view.write_text(json.dumps(view), encoding="utf-8")
    assert store.json("--marker", str(marker), "verify", str(saved_view)) == {"verified": True}
    view["app"] = "app-b"
    saved_view.write_text(json.dumps(view), encoding="utf-8")
    assert store.run("--marker", str(marker), "verify", str(saved_view)).returncode == 1
    assert json.loads(marker.read_text()) == proof
