"""Failures from the current CLI and its process boundary."""
from typing import Optional


class SaucepanError(RuntimeError):
    """Preserve diagnostics without assigning obsolete workspace error categories."""

    def __init__(self, message: str, exit_code: Optional[int] = None,
                 stdout: str = "", stderr: str = "", code: Optional[str] = None):
        super().__init__(message)
        self.exit_code = exit_code
        self.stdout = stdout
        self.stderr = stderr
        self.code = code
