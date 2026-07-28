import subprocess
from pathlib import Path

import pytest

from saucepan_sdk import Workspace

from support import make_git_repository


def test_sauce_path_delegates_slash_containing_target_to_the_cli(
    saucepan_binary: Path,
    workspace: Path,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    source_root = tmp_path / "sources"
    make_git_repository(source_root / "repo.git")
    monkeypatch.setenv("GIT_CONFIG_COUNT", "1")
    monkeypatch.setenv(
        "GIT_CONFIG_KEY_0", "url.{}/.insteadOf".format(source_root.as_uri())
    )
    monkeypatch.setenv("GIT_CONFIG_VALUE_0", "https://github.com/owner/")
    (workspace / "saucepan.toml").write_text(
        '[github]\nbinary = "git"\n',
        encoding="utf-8",
    )
    client = Workspace(workspace, binary=saucepan_binary)
    client.install("owner/repo")
    sauce = client.sauces[0]
    direct = subprocess.run(
        [str(saucepan_binary), str(workspace), "path", sauce.name],
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )

    assert sauce.path == Path(direct.stdout.strip())
    assert sauce.path.is_dir()
