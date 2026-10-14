"""Errors never include argv, payloads or captured process diagnostics."""


class TransportError(Exception):
    def __init__(self, kind: str, *, started: bool = False, indeterminate: bool = False):
        self.kind = kind
        self.started = started
        self.indeterminate = indeterminate
        super().__init__(f"Saucepan transport: {kind}")


class SaucepanError(Exception):
    def __init__(self, code: str, message: str, details: dict[str, str]):
        self.code = code
        self.message = message
        self.details = details
        # The structured fields are deliberately available, but not echoed to logs.
        super().__init__(f"Saucepan: {code}")
