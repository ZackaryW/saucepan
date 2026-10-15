"""Validate the lifecycle envelope, without interpreting or retrying operations."""
from .errors import TransportError

def receipt_result(value: object, exit_code: int | None, mode: str) -> object:
    records = [value] if mode == 'operation' else value
    if not isinstance(records, list):
        raise TransportError('protocol', started=True)
    for item in records:
        if not isinstance(item, dict):
            raise TransportError('protocol', started=True)
        if (any(not isinstance(item.get(k), str) for k in ('operation', 'app', 'claim', 'fingerprint'))
            or item.get('status') not in ('preparing', 'commit_intent', 'cleanup_pending', 'complete', 'failed', 'conflict')
            or type(item.get('changed')) is not bool or not isinstance(item.get('action'), dict)
            or item['action'].get('type') not in ('install', 'update', 'shim', 'uninstall')
            or any(not isinstance(item.get(k), list) or any(not isinstance(v, str) for v in item[k]) for k in ('pending', 'leftovers'))
            or not isinstance(item.get('shims'), list)):
            raise TransportError('protocol', started=True)
        for key, expected in (('candidate', str), ('desired', dict), ('desired_registry', dict), ('source_policy', dict), ('reason', dict), ('expected_revision', int)):
            if key not in item or (item[key] is not None and type(item[key]) is not expected):
                raise TransportError('protocol', started=True)
    if mode == 'inspection':
        if exit_code != 0:
            raise TransportError('protocol', started=True)
        return value
    expected_code = 0 if all(item['status'] == 'complete' for item in records) else 2
    if exit_code != expected_code:
        raise TransportError('protocol', started=True)
    return {'exit_code': exit_code, 'result': value}
