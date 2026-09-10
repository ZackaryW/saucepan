"""Independent real-CLI lifecycle for Python SDK behavior scenarios."""
import os
import subprocess
import tempfile
from pathlib import Path

from saucepan_sdk import Saucepan

REPOSITORY_ROOT = Path(__file__).resolve().parents[4]


def before_all(context):
    supplied = os.environ.get("SAUCEPAN_TEST_BINARY")
    if supplied:
        context.binary = Path(supplied).resolve(strict=True)
    else:
        subprocess.run(["cargo", "build", "--locked"], cwd=REPOSITORY_ROOT, check=True)
        context.binary = REPOSITORY_ROOT / "target/debug" / ("saucepan.exe" if os.name == "nt" else "saucepan")


def before_scenario(context, _scenario):
    context.temporary_directory = tempfile.TemporaryDirectory(prefix="sp-")
    context.test_root = Path(context.temporary_directory.name)
    context.store = Saucepan(binary=context.binary, test_root=context.test_root / "store",
                            test_key=os.urandom(32).hex())
    context.store.init()


def after_scenario(context, _scenario):
    context.temporary_directory.cleanup()
