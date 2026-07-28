import json
import subprocess
from pathlib import Path


def test_real_binary_reads_empty_workspace_index(
    saucepan_binary: Path, workspace: Path
) -> None:
    result = subprocess.run(
        [str(saucepan_binary), str(workspace), "cat", "index"],
        check=True,
        capture_output=True,
        text=True,
    )

    assert json.loads(result.stdout) == []
