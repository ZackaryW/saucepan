import { TransportError } from './errors.js';
function object(v: unknown): v is Record<string, unknown> { return v !== null && typeof v === 'object' && !Array.isArray(v); }
/** Envelope validation only. Retrying and recovery always belong to the caller. */
export function receiptResult(value: unknown, exitCode: number | null, mode: 'operation' | 'reconcile' | 'inspection'): unknown {
  const records = mode === 'operation' ? [value] : value;
  if (!Array.isArray(records)) throw new TransportError('protocol', true);
  for (const item of records) {
    if (!object(item) || !['operation', 'app', 'claim', 'fingerprint'].every(k => typeof item[k] === 'string')
      || !['preparing', 'commit_intent', 'cleanup_pending', 'complete', 'failed', 'conflict'].includes(String(item.status))
      || typeof item.changed !== 'boolean' || !object(item.action) || !['install', 'update', 'shim', 'uninstall'].includes(String(item.action.type))
      || !['pending', 'leftovers'].every(k => Array.isArray(item[k]) && item[k].every((v: unknown) => typeof v === 'string'))
      || !Array.isArray(item.shims)) throw new TransportError('protocol', true);
    for (const key of ['candidate', 'desired', 'desired_registry', 'source_policy', 'reason', 'expected_revision']) {
      const v = item[key];
      if (!(key in item) || (v !== null && !(key === 'candidate' ? typeof v === 'string' : key === 'expected_revision' ? Number.isSafeInteger(v) : object(v)))) throw new TransportError('protocol', true);
    }
  }
  if (mode === 'inspection') {
    if (exitCode !== 0) throw new TransportError('protocol', true);
    return value;
  }
  if (exitCode !== (records.every(item => item.status === 'complete') ? 0 : 2)) throw new TransportError('protocol', true);
  return { exit_code: exitCode, result: value };
}
