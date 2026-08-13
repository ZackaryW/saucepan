import importlib.util
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]


def load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_root_feature_lifecycle_exposes_the_planned_contract() -> None:
    lifecycle = load_module(
        REPOSITORY_ROOT / "features" / "support" / "lifecycle.py",
        "root_feature_lifecycle",
    )

    for name in (
        "repository_root",
        "built_binary",
        "before_all",
        "before_scenario",
        "after_scenario",
        "write_workspace_config",
        "run_cli",
        "make_git_repository",
        "commit_repository",
        "tag_repository",
        "read_workspace_json",
        "write_bucket",
    ):
        assert callable(getattr(lifecycle, name))


def test_sdk_feature_lifecycle_exposes_the_planned_contract() -> None:
    lifecycle = load_module(
        REPOSITORY_ROOT
        / "sdk"
        / "python"
        / "features"
        / "support"
        / "lifecycle.py",
        "sdk_feature_lifecycle",
    )

    for name in (
        "before_all",
        "before_scenario",
        "after_scenario",
        "make_index_repository",
        "advance_index",
        "write_sdk_state_fixture",
    ):
        assert callable(getattr(lifecycle, name))
