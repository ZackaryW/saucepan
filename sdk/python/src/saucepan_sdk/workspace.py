"""Workspace entry point for the saucepan SDK."""

import json
import os
import shutil
from pathlib import Path
from typing import Any, List, Optional, Tuple, Union

from ._runner import CommandRunner
from .entities import Bucket, Sauce
from .errors import InternalError, SaucepanError


Pathish = Union[str, os.PathLike]


class Workspace:
    """A saucepan workspace driven through one resolved executable."""

    def __init__(self, root: Pathish, binary: Optional[Pathish] = None) -> None:
        self.root = Path(root)
        self.binary = self._resolve_binary(binary)
        self._runner = CommandRunner(self.binary, self.root)
        self._index_cache: Optional[List[Any]] = None
        self._bucket_cache: Optional[List[Any]] = None

    @property
    def sauces(self) -> Tuple[Sauce, ...]:
        """Return installed sauces as snapshots of the full index."""
        if self._index_cache is None:
            self._index_cache = self._runner.run_json("cat", "index")
        return tuple(Sauce(self, entry) for entry in self._index_cache)

    @property
    def buckets(self) -> Tuple[Bucket, ...]:
        """Return registered bucket sources as entities."""
        if self._bucket_cache is None:
            self._bucket_cache = self._runner.run_json("cat", "buckets")
        return tuple(Bucket(self, entry["url"]) for entry in self._bucket_cache)

    def install(self, target: str, reference: Optional[str] = None) -> Sauce:
        """Install a target and return its manifest-named sauce entity."""
        args = ["install", target]
        if reference is not None:
            args.extend(["--ref", reference])
        self._mutate(*args)
        entries = self.cat_index()
        for entry in reversed(entries):
            manifest = entry["sauce"]
            origin = entry.get("repo") or entry.get("url")
            if (
                manifest["name"] == target
                or origin == target
                or (origin is not None and origin.rstrip("/").endswith("/" + target))
            ):
                return Sauce(self, entry)
        raise InternalError(5, "install succeeded but its index entry was not found")

    def list(self) -> List[Any]:
        """Return the parsed entries emitted by ``list --json``."""
        return self._runner.run_ndjson("list", "--json")

    def path(self, name: str) -> Path:
        """Return the path reported by the executable for an installed sauce."""
        return Path(self._runner.run_text("path", name).strip())

    def cat_index(self) -> List[Any]:
        """Return the parsed full local index."""
        return self._runner.run_json("cat", "index")

    def cat_buckets(self) -> List[Any]:
        """Return the parsed bucket registry."""
        return self._runner.run_json("cat", "buckets")

    def cat_sauce(self, name: str) -> Any:
        """Return one parsed index entry."""
        return self._runner.run_json("cat", "sauce", name)

    def cat_bucket(self, url: str) -> List[Any]:
        """Return the parsed stubs from one bucket document."""
        return self._runner.run_json("cat", "bucket", url)

    def add_bucket(self, url: str, reference: Optional[str] = None) -> Bucket:
        """Register a bucket, optionally pinned to a ref, and return its entity."""
        args = ["bucket", "add", url]
        if reference is not None:
            args.extend(["--ref", reference])
        self._mutate(*args)
        return Bucket(self, url)

    def refresh_bucket(self, url: str) -> Bucket:
        """Refresh a registered bucket and return its entity."""
        self._mutate("bucket", "refresh", url)
        return Bucket(self, url)

    def remove_bucket(self, url: str) -> None:
        """Remove a registered bucket."""
        self._mutate("bucket", "remove", url)

    def list_buckets(self) -> List[Any]:
        """Return parsed entries emitted by ``bucket list --json``."""
        return self._runner.run_ndjson("bucket", "list", "--json")

    def search(self, jq_filter: str) -> List[Any]:
        """Return the raw parsed JSON values emitted by a jq predicate."""
        output = self._runner.run_text("search", jq_filter).strip()
        if not output or output in {"no buckets registered", "no matches"}:
            return []
        return [json.loads(line) for line in output.splitlines() if line.strip()]

    def refresh(self) -> None:
        """Discard cached state and eagerly reread the full sauce index."""
        self._invalidate_cache()
        self._index_cache = self._runner.run_json("cat", "index")

    def _mutate(self, *args: str) -> str:
        output = self._runner.run_text(*args)
        self._invalidate_cache()
        return output

    def _invalidate_cache(self) -> None:
        self._index_cache = None
        self._bucket_cache = None

    @staticmethod
    def _resolve_binary(binary: Optional[Pathish]) -> str:
        if binary is None:
            resolved = shutil.which("saucepan")
            if resolved is None:
                raise SaucepanError(
                    None,
                    "cannot run saucepan binary 'saucepan': not found on PATH",
                )
            return resolved

        explicit = os.fspath(binary)
        if not Path(explicit).is_file() or not os.access(explicit, os.X_OK):
            raise SaucepanError(
                None,
                "cannot run saucepan binary '{}': file is missing or not executable".format(
                    explicit
                ),
            )
        return explicit
