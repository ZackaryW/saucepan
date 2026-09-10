import os
import subprocess
import tempfile
from pathlib import Path

import pytest

from central_store_support import CentralTestStore


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]


@pytest.fixture(scope="session")
def saucepan_test_binary() -> Path:
    supplied = os.environ.get("SAUCEPAN_TEST_BINARY")
    if supplied:
        binary = Path(supplied).resolve(strict=True)
    else:
        if not (REPOSITORY_ROOT / "src" / "core" / "mod.rs").is_file():
            pytest.fail("Set SAUCEPAN_TEST_BINARY to a compatible central-store CLI")
        subprocess.run(
            ["cargo", "build", "--locked", "--bin", "saucepan"],
            cwd=REPOSITORY_ROOT, check=True,
        )
        binary = REPOSITORY_ROOT / "target" / "debug" / ("saucepan.exe" if os.name == "nt" else "saucepan")
    help_output = subprocess.run([str(binary), "--help"], capture_output=True, check=True, timeout=10)
    if b"--test-root" not in help_output.stdout:
        pytest.fail("SAUCEPAN_TEST_BINARY must support the current --test-root protocol")
    return binary


@pytest.fixture
def central_store(saucepan_test_binary: Path):
    # Leave room for source/snapshot IDs under Windows filesystem path limits.
    with tempfile.TemporaryDirectory(prefix="sauce-") as temporary:
        yield CentralTestStore.create(saucepan_test_binary, Path(temporary) / "central")


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
