import shutil
import subprocess
import sys
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
SDK_ROOT = REPOSITORY_ROOT / "sdk" / "python"


def test_sdk_materializes_independently_and_constructs_workspace(
    saucepan_binary: Path,
    workspace: Path,
    tmp_path: Path,
) -> None:
    consumer = tmp_path / "sdk-consumer"
    materialized_sdk = consumer / "sdk" / "python"
    shutil.copytree(
        SDK_ROOT,
        materialized_sdk,
        ignore=shutil.ignore_patterns(".venv", "__pycache__", ".pytest_cache"),
    )

    script = """
import sys
from pathlib import Path

sdk_root = Path(sys.argv[1]).resolve()
sys.path.insert(0, str(sdk_root / "src"))

import saucepan_sdk
from saucepan_sdk import Workspace

Path(saucepan_sdk.__file__).resolve().relative_to(sdk_root / "src")
client = Workspace(sys.argv[2], binary=sys.argv[3])
assert Path(client.binary).resolve() == Path(sys.argv[3]).resolve()
"""
    subprocess.run(
        [
            sys.executable,
            "-I",
            "-c",
            script,
            str(materialized_sdk),
            str(workspace),
            str(saucepan_binary),
        ],
        cwd=consumer,
        check=True,
        capture_output=True,
        text=True,
    )
