import os
import subprocess
from pathlib import Path

import pytest


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]


@pytest.fixture(scope="session")
def saucepan_binary() -> Path:
    subprocess.run(
        ["cargo", "build", "--bin", "saucepan"],
        cwd=REPOSITORY_ROOT,
        check=True,
    )
    executable = "saucepan.exe" if os.name == "nt" else "saucepan"
    return REPOSITORY_ROOT / "target" / "debug" / executable


@pytest.fixture
def workspace(tmp_path: Path) -> Path:
    workspace = tmp_path / "workspace"
    workspace.mkdir()
    (workspace / "saucepan.toml").write_text("[local]\n", encoding="utf-8")
    return workspace
