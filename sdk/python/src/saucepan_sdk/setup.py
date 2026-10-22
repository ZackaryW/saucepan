"""Decode setup separately from application lifecycle action receipts."""
from uuid import UUID
from .errors import TransportError


def _uuid(value: object) -> bool:
    if not isinstance(value, str):
        return False
    try:
        return str(UUID(value)) == value.lower()
    except ValueError:
        return False


def setup_result(value: object, exit_code: int | None, mode: str) -> object:
    def fail() -> None:
        raise TransportError('protocol', started=True)

    if not isinstance(value, dict):
        fail()
    assert isinstance(value, dict)
    if type(value.get('schema_version')) is not int or value['schema_version'] != 1 or not isinstance(value.get('root'), str) or not value['root']:
        fail()
    for key in ('app', 'claim', 'output', 'operation'):
        if key not in value or (value[key] is not None and not _uuid(value[key])):
            fail()
    for key in ('launcher_path', 'installed_version'):
        if key not in value or (value[key] is not None and (not isinstance(value[key], str) or not value[key])):
            fail()
    if not isinstance(value.get('diagnostics'), list) or any(not isinstance(v, str) for v in value['diagnostics']):
        fail()
    ready = False
    if mode == 'setup_status':
        if exit_code != 0 or value.get('state') not in ('absent', 'unmanaged', 'ready', 'pending', 'conflict'):
            fail()
        ready = value['state'] == 'ready'
    else:
        if value.get('status') not in ('preparing', 'commit_intent', 'cleanup_pending', 'complete', 'failed', 'conflict') or type(value.get('changed')) is not bool or not _uuid(value['operation']):
            fail()
        if 'lifecycle_operation' not in value or (value['lifecycle_operation'] is not None and not _uuid(value['lifecycle_operation'])):
            fail()
        if 'retry_shim' not in value or (value['retry_shim'] is not None and not isinstance(value['retry_shim'], str)):
            fail()
        if not isinstance(value.get('backup_paths'), list) or any(not isinstance(v, str) for v in value['backup_paths']):
            fail()
        ready = value['status'] == 'complete'
        if exit_code != (0 if ready else 2):
            fail()
    if ready and any(value[k] is None for k in ('app', 'claim', 'output', 'launcher_path', 'installed_version')):
        fail()
    return value if mode == 'setup_status' else {'exit_code': exit_code, 'result': value}
