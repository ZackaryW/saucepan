"""Snapshot entities exposed by the saucepan SDK."""

from pathlib import Path
from typing import Any, List, Mapping, Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from .workspace import Workspace


class Sauce:
    """One installed sauce as reported by the workspace index."""

    def __init__(self, workspace: "Workspace", entry: Mapping[str, Any]) -> None:
        self._workspace = workspace
        self._replace(entry)

    def _replace(self, entry: Mapping[str, Any]) -> None:
        manifest = entry["sauce"]
        self.name = manifest["name"]
        self.version = manifest["version"]
        self.description = manifest["description"]
        self.reference: Optional[str] = entry.get("reference")
        self.resolved_commit: Optional[str] = entry.get("resolved_commit")

    def update(self) -> "Sauce":
        """Update and refresh this snapshot in place."""
        self._workspace._mutate("update", self.name)
        entry = self._workspace._runner.run_json("cat", "sauce", self.name)
        self._replace(entry)
        return self

    def uninstall(self) -> None:
        """Uninstall this sauce from its workspace."""
        self._workspace._mutate("uninstall", self.name)

    @property
    def path(self) -> Path:
        """Return the artifact path reported by the saucepan executable."""
        output = self._workspace._runner.run_text("path", self.name)
        return Path(output.strip())


class Bucket:
    """One registered bucket source."""

    def __init__(self, workspace: "Workspace", url: str) -> None:
        self._workspace = workspace
        self.url = url

    def remove(self) -> None:
        """Remove this bucket from its workspace."""
        self._workspace.remove_bucket(self.url)

    def stubs(self) -> List[Any]:
        """Return parsed sauce stubs from this bucket."""
        return self._workspace.cat_bucket(self.url)
