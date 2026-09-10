import ast
import shutil
import subprocess
import sys
from pathlib import Path

SDK_ROOT = Path(__file__).resolve().parents[1]


def test_sdk_runs_independently_without_site_packages(saucepan_test_binary, tmp_path):
    sdk = tmp_path / "vendored"
    shutil.copytree(SDK_ROOT / "src", sdk)
    script = """
import sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
from saucepan_sdk import Saucepan
client = Saucepan(binary=sys.argv[2], test_root=sys.argv[3], test_key="23" * 32)
assert client.init() == {"created": True}
app = client.for_app(client.register("consumer"))
assert app.view()["entries"] == {}
"""
    subprocess.run([sys.executable, "-I", "-S", "-c", script, str(sdk),
                    str(saucepan_test_binary), str(tmp_path / "store")],
                   cwd=tmp_path, check=True, capture_output=True, text=True)


def test_runtime_imports_are_standard_library_only():
    # Import inventory is deliberately explicit and works on Python 3.9 too.
    allowed = {"copy", "json", "math", "os", "re", "subprocess", "tempfile", "pathlib", "typing"}
    for path in (SDK_ROOT / "src/saucepan_sdk").glob("*.py"):
        for node in ast.walk(ast.parse(path.read_text(encoding="utf-8"))):
            if isinstance(node, ast.Import):
                assert all(alias.name.split(".")[0] in allowed for alias in node.names), path
            elif isinstance(node, ast.ImportFrom) and not node.level:
                assert node.module.split(".")[0] in allowed, path
