import { TransportError } from './errors.js';
const uuid = (v: unknown): boolean => typeof v === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(v);
const strings = (v: unknown): boolean => Array.isArray(v) && v.every(s => typeof s === 'string');
export function setupResult(value: unknown, exitCode: number | null, mode: 'setup' | 'setup_status'): unknown {
  const fail = (): never => { throw new TransportError('protocol', true); };
  if (!value || typeof value !== 'object' || Array.isArray(value)) fail();
  const item = value as Record<string, unknown>;
  if (item.schema_version !== 1 || typeof item.root !== 'string' || !item.root || !strings(item.diagnostics)) fail();
  for (const key of ['app', 'claim', 'output', 'operation']) {
    if (!(key in item) || (item[key] !== null && !uuid(item[key]))) fail();
  }
  for (const key of ['launcher_path', 'installed_version']) {
    if (!(key in item) || (item[key] !== null && (typeof item[key] !== 'string' || !item[key]))) fail();
  }
  let ready: boolean;
  if (mode === 'setup_status') {
    if (exitCode !== 0 || !['absent','unmanaged','ready','pending','conflict'].includes(String(item.state))) fail();
    ready = item.state === 'ready';
  } else {
    if (!['preparing','commit_intent','cleanup_pending','complete','failed','conflict'].includes(String(item.status))
      || typeof item.changed !== 'boolean' || !uuid(item.operation) || !strings(item.backup_paths)) fail();
    if (!('lifecycle_operation' in item) || (item.lifecycle_operation !== null && !uuid(item.lifecycle_operation))) fail();
    if (!('retry_shim' in item) || (item.retry_shim !== null && typeof item.retry_shim !== 'string')) fail();
    ready = item.status === 'complete';
    if (exitCode !== (ready ? 0 : 2)) fail();
  }
  if (ready && ['app','claim','output','launcher_path','installed_version'].some(k => item[k] === null)) fail();
  return mode === 'setup_status' ? value : { exit_code: exitCode, result: value };
}
