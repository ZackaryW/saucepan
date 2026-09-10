import functools
import http.server
import threading
import zipfile
from pathlib import Path

from saucepan_sdk import Saucepan


def test_file_and_zip_downloads_through_client(central_store, tmp_path):
    (tmp_path / "file.txt").write_text("download ü", encoding="utf-8")
    with zipfile.ZipFile(tmp_path / "archive.zip", "w") as archive:
        archive.writestr("package/assets/file.txt", "archive data")
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0),
        functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(tmp_path)))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        store = Saucepan(binary=central_store.binary, test_root=central_store.root, test_key=central_store.key)
        app = store.for_app(store.register("downloads"))
        url = f"http://127.0.0.1:{server.server_port}"
        result = app.acquire({"source": {"provider": "url", "url": url + "/file.txt",
            "download": {"format": "file", "name": "saved.txt"}}})
        assert Path(result["directory"], "saved.txt").read_text(encoding="utf-8") == "download ü"
        zipped = app.acquire({"source": {"provider": "url", "url": url + "/archive.zip",
            "download": {"format": "zip"}}, "folder": "package/assets"})
        assert Path(zipped["directory"], "file.txt").read_text() == "archive data"
        assert len(app.view()["entries"]) == 2
    finally:
        server.shutdown()
        server.server_close()
        thread.join()
