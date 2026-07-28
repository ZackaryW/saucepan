import os
import subprocess
import tempfile
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]


def before_all(context):
    subprocess.run(
        ["cargo", "build", "--bin", "saucepan"],
        cwd=REPOSITORY_ROOT,
        check=True,
    )
    executable = "saucepan.exe" if os.name == "nt" else "saucepan"
    context.saucepan_binary = REPOSITORY_ROOT / "target" / "debug" / executable


def before_scenario(context, _scenario):
    context.temporary_directory = tempfile.TemporaryDirectory()
    context.test_root = Path(context.temporary_directory.name)
    context.saved_git_environment = {
        key: os.environ.get(key)
        for key in ("GIT_CONFIG_COUNT", "GIT_CONFIG_KEY_0", "GIT_CONFIG_VALUE_0")
    }


def after_scenario(context, _scenario):
    for key, value in context.saved_git_environment.items():
        if value is None:
            os.environ.pop(key, None)
        else:
            os.environ[key] = value
    context.temporary_directory.cleanup()
