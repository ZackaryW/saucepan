import shutil
import subprocess
import sys
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
SDK_ROOT = REPOSITORY_ROOT / "sdk" / "python"
RESOLVER = REPOSITORY_ROOT / "resolvers" / "python" / "saucepan_resolver.py"


def test_sdk_materializes_without_resolver_and_constructs_workspace(
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

    assert not (consumer / "resolvers").exists()

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


def test_resolver_materializes_without_sdk_and_remains_runnable(
    tmp_path: Path,
) -> None:
    consumer = tmp_path / "resolver-consumer"
    resolver_directory = consumer / "resolvers" / "python"
    resolver_directory.mkdir(parents=True)
    materialized_resolver = resolver_directory / RESOLVER.name
    shutil.copy2(RESOLVER, materialized_resolver)

    assert not (consumer / "sdk").exists()

    import_script = """
import sys

sys.path.insert(0, sys.argv[1])
import saucepan_resolver

assert saucepan_resolver._asset_name("Linux", "x86_64") == (
    "saucepan-x86_64-unknown-linux-musl"
)
"""
    subprocess.run(
        [sys.executable, "-I", "-c", import_script, str(resolver_directory)],
        cwd=consumer,
        check=True,
        capture_output=True,
        text=True,
    )

    help_result = subprocess.run(
        [sys.executable, "-I", str(materialized_resolver)],
        cwd=consumer,
        check=False,
        capture_output=True,
        text=True,
    )

    assert help_result.returncode == 2
    assert help_result.stdout == ""
    assert "usage: saucepan_resolver.py <version> [dest]" in help_result.stderr
