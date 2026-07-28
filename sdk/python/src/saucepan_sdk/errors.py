"""Exceptions raised for saucepan command failures."""

from typing import Optional


class SaucepanError(RuntimeError):
    """Base error carrying the CLI exit code and captured standard error."""

    def __init__(self, exit_code: Optional[int], stderr: str) -> None:
        self.exit_code = exit_code
        self.stderr = stderr
        super().__init__(stderr.strip())


class NotFound(SaucepanError):
    """The requested object does not exist (exit code 1)."""


class SourceError(SaucepanError):
    """A configured source failed (exit code 2)."""


class ConfigError(SaucepanError):
    """The workspace configuration is invalid (exit code 3)."""


class Conflict(SaucepanError):
    """The requested mutation conflicts with existing state (exit code 4)."""


class InternalError(SaucepanError):
    """Saucepan failed outside the public error categories (exit code 5)."""
