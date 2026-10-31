"""One loopback provider fixture for every route the manager and installers use.

State is declarative: repositories hold releases, commits (file trees) and refs;
installer downloads are keyed by tag. Routes under ``base``:

- GitHub API: ``/repos/{o}/{r}/releases/latest|tags/{tag}|assets/{id}``,
  ``/repos/{o}/{r}/commits/{ref}``, ``/repos/{o}/{r}/zipball/{commit}``
- GitHub raw: ``/github-raw/{o}/{r}/{commit}/{path}``
- Git platform raw: ``/git/{repo}/-/raw/{ref}/{path}`` (GitLab) and
  ``/git/{repo}/raw/{ref}/{path}`` (Gitea)
- Installer downloads: ``/releases/{tag}/{file}``

Release ``browser_download_url`` values answer 410 so a regression away from the
asset API fails loudly. Every request is logged with its credential headers.
"""
from __future__ import annotations

from dataclasses import dataclass, field
import hashlib
import http.server
import io
import json
import threading
from urllib.parse import unquote, urlsplit
import zipfile


@dataclass
class Release:
    id: int
    tag: str
    assets: dict[str, tuple[int, bytes]]
    prerelease: bool = False
    draft: bool = False


@dataclass
class Repository:
    releases: list[Release] = field(default_factory=list)
    refs: dict[str, str] = field(default_factory=dict)
    commits: dict[str, dict[str, bytes]] = field(default_factory=dict)


@dataclass(frozen=True)
class Request:
    method: str
    path: str
    authorization: str | None
    private_token: str | None
    accept: str | None


class Forge:
    def __init__(self) -> None:
        self.repositories: dict[str, Repository] = {}
        self.installers: dict[str, dict[str, bytes]] = {}
        self.requests: list[Request] = []
        self.fail = False
        self._assets: dict[int, bytes] = {}
        self._next_id = 1
        self._lock = threading.Lock()
        self._gate: threading.Event | None = None
        self.held = threading.Event()
        self._server: http.server.ThreadingHTTPServer | None = None

    # -- declarative state -------------------------------------------------
    def repository(self, name: str) -> Repository:
        return self.repositories.setdefault(name.lower(), Repository())

    def release(self, repo: str, tag: str, assets: dict[str, bytes], *,
                prerelease: bool = False, draft: bool = False) -> Release:
        with self._lock:
            ids = {}
            for name, data in assets.items():
                ids[name] = (self._next_id, data)
                self._assets[self._next_id] = data
                self._next_id += 1
            release = Release(self._next_id, tag, ids, prerelease, draft)
            self._next_id += 1
        self.repository(repo).releases.append(release)
        return release

    def commit(self, repo: str, files: dict[str, bytes], *refs: str) -> str:
        digest = hashlib.sha1()
        for path in sorted(files):
            digest.update(path.encode() + b"\0" + files[path] + b"\0")
        commit = digest.hexdigest()
        repository = self.repository(repo)
        repository.commits[commit] = dict(files)
        for ref in refs:
            repository.refs[ref] = commit
        return commit

    def installer(self, tag: str, files: dict[str, bytes]) -> None:
        self.installers.setdefault(tag, {}).update(files)

    def hold_discovery(self) -> threading.Event:
        """Block release discovery until the returned event is set; ``held`` is set once a request waits."""
        self._gate = threading.Event()
        self.held.clear()
        return self._gate

    # -- endpoints ---------------------------------------------------------
    @property
    def base(self) -> str:
        assert self._server is not None, "forge not started"
        host, port = self._server.server_address[:2]
        return f"http://127.0.0.1:{port}"

    @property
    def github_api(self) -> str:
        return self.base

    @property
    def github_raw(self) -> str:
        return self.base + "/github-raw"

    @property
    def git_raw(self) -> str:
        return self.base + "/git"

    @property
    def releases_base(self) -> str:
        return self.base + "/releases"

    def requested(self, prefix: str = "") -> list[Request]:
        with self._lock:
            return [r for r in self.requests if r.path.startswith(prefix)]

    # -- lifecycle ---------------------------------------------------------
    def start(self) -> "Forge":
        forge = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                self._serve(body=True)

            def do_HEAD(self):
                self._serve(body=False)

            def _serve(self, body):
                with forge._lock:
                    forge.requests.append(Request(self.command, self.path,
                                                  self.headers.get("Authorization"),
                                                  self.headers.get("PRIVATE-TOKEN"),
                                                  self.headers.get("Accept")))
                status, data, kind = forge._route(unquote(urlsplit(self.path).path))
                self.send_response(status)
                self.send_header("Content-Length", str(len(data)))
                self.send_header("Content-Type", kind)
                self.send_header("ETag", '"' + hashlib.sha256(data).hexdigest()[:16] + '"')
                self.end_headers()
                if body:
                    self.wfile.write(data)

            def log_message(self, *_):
                pass

        self._server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=self._server.serve_forever, daemon=True).start()
        return self

    def close(self) -> None:
        if self._gate is not None:
            self._gate.set()
        if self._server is not None:
            self._server.shutdown()
            self._server.server_close()
            self._server = None

    def __enter__(self) -> "Forge":
        return self.start()

    def __exit__(self, *_) -> None:
        self.close()

    # -- routing -----------------------------------------------------------
    def _route(self, path: str) -> tuple[int, bytes, str]:
        parts = [p for p in path.split("/") if p]
        if parts[:1] == ["repos"] and len(parts) >= 4:
            return self._github(parts[1] + "/" + parts[2], parts[3:])
        if parts[:1] == ["github-raw"] and len(parts) >= 5:
            repo, commit, rel = parts[1] + "/" + parts[2], parts[3], "/".join(parts[4:])
            return self._file(repo, commit, rel)
        if parts[:1] == ["git"]:
            return self._platform_raw(parts[1:])
        if parts[:1] == ["releases"] and len(parts) == 3:
            data = self.installers.get(parts[1], {}).get(parts[2])
            return (200, data, "application/octet-stream") if data is not None else _missing()
        if parts[:1] == ["browser"]:
            return 410, b"browser download URLs are not used", "text/plain"
        return _missing()

    def _github(self, repo: str, rest: list[str]) -> tuple[int, bytes, str]:
        repository = self.repositories.get(repo.lower())
        if rest[:2] == ["releases", "assets"] and len(rest) == 3:
            data = self._assets.get(int(rest[2])) if rest[2].isdigit() else None
            return (200, data, "application/octet-stream") if data is not None else _missing()
        if self._gate is not None and rest[:1] == ["releases"]:
            self.held.set()
            self._gate.wait(30)
        if self.fail:
            return 503, b"fixture outage", "text/plain"
        if repository is None:
            return _missing()
        if rest[:1] == ["releases"]:
            if rest[1:] == ["latest"]:
                stable = [r for r in repository.releases if not r.prerelease and not r.draft]
                release = stable[-1] if stable else None
            elif rest[1:2] == ["tags"] and len(rest) == 3:
                release = next((r for r in reversed(repository.releases) if r.tag == rest[2]), None)
            else:
                release = None
            return (200, self._release_json(release), "application/json") if release else _missing()
        if rest[:1] == ["commits"] and len(rest) >= 2:
            commit = self._resolve(repository, "/".join(rest[1:]))
            return (200, commit.encode(), "application/vnd.github.sha") if commit else _missing()
        if rest[:1] == ["zipball"] and len(rest) == 2:
            files = repository.commits.get(rest[1])
            if files is None:
                return _missing()
            owner, name = repo.lower().split("/")
            return 200, _zip(f"{owner}-{name}-{rest[1][:7]}", files), "application/zip"
        return _missing()

    def _platform_raw(self, parts: list[str]) -> tuple[int, bytes, str]:
        for marker in (["-", "raw"], ["raw"]):
            for i in range(1, len(parts)):
                if parts[i:i + len(marker)] == marker and len(parts) > i + len(marker) + 1:
                    repo = "/".join(parts[:i])
                    ref = parts[i + len(marker)]
                    rel = "/".join(parts[i + len(marker) + 1:])
                    repository = self.repositories.get(repo.lower())
                    commit = self._resolve(repository, ref) if repository else None
                    return self._file(repo, commit, rel) if commit else _missing()
        return _missing()

    def _resolve(self, repository: Repository, ref: str) -> str | None:
        if ref in repository.commits:
            return ref
        return repository.refs.get(ref)

    def _file(self, repo: str, commit: str, rel: str) -> tuple[int, bytes, str]:
        repository = self.repositories.get(repo.lower())
        data = repository.commits.get(commit, {}).get(rel) if repository else None
        return (200, data, "application/octet-stream") if data is not None else _missing()

    def _release_json(self, release: Release) -> bytes:
        return json.dumps({
            "id": release.id, "tag_name": release.tag, "draft": release.draft,
            "prerelease": release.prerelease,
            "assets": [{"id": asset_id, "name": name,
                        "browser_download_url": f"{self.base}/browser/{asset_id}",
                        "digest": "sha256:" + hashlib.sha256(data).hexdigest()}
                       for name, (asset_id, data) in release.assets.items()],
        }).encode()


def _missing() -> tuple[int, bytes, str]:
    return 404, b"not found", "text/plain"


def _zip(root: str, files: dict[str, bytes]) -> bytes:
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as archive:
        archive.writestr(root + "/", b"")
        for path, data in sorted(files.items()):
            archive.writestr(f"{root}/{path}", data)
    return buffer.getvalue()
