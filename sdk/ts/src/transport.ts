import type { Call, Config, RunOptions, RunResult, Transport } from './contracts.js';
import { Payload } from './contracts.js';
import { SaucepanError, TransportError } from './errors.js';
import { decode } from './utils/json.js';
import { payloadFiles } from './utils/payloads.js';
import { invoke } from './utils/process.js';

function object(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function checkShape(value: unknown, call: Call): void {
  if (call.shape === 'array') {
    if (!Array.isArray(value) || !value.every(item => call.argv[0] === 'register-policies' ? typeof item === 'string' : object(item))) throw new TransportError('protocol', true);
    return;
  }
  if (!object(value)) throw new TransportError('protocol', true);
  const strings = new Set(['uuid','app','root','integrity','type','entry_uuid','directory','path','verification','released','removed','state']);
  const objects = new Set(['source','claim','view','acquired','entry','request','descriptor','ownership','definition','group','resources','paths','outputs','context','deferred','pending']);
  const arrays = new Set(['sources','claims','policy_refs','reclaimed','stale']);
  for (const key of call.required ?? []) {
    const item = value[key];
    if (!(key in value) || (strings.has(key) && typeof item !== 'string') || (objects.has(key) && !object(item))
      || (arrays.has(key) && !Array.isArray(item)) || (['revision','generation','policy_revision'].includes(key) && !Number.isSafeInteger(item))
      || (key === 'purged' && typeof item !== 'boolean')) throw new TransportError('protocol', true);
  }
  if (call.required?.includes('type') && !['application','output','retrieval','acquisition','access_policy','source_visibility_policy','claim_visibility_policy',
    'cache_policy','refresh_policy','stale_policy','verification_policy','github_release_asset_filter','selection_policy','identical_pull_policy','update_policy',
    'revision_policy','reclaim_policy','useable_app_policy','pin_policy','policy_constraints'].includes(String(value.type))) throw new TransportError('protocol', true);
}

export class NodeTransport implements Transport {
  async run(config: Config, call: Call, options: RunOptions): Promise<RunResult> {
    const prefix = ['--error-format', 'json'];
    for (const [flag, value] of [['--root', config.root], ['--as-app', config.app], ['--credential-file', config.credentialFile]]) {
      if (value !== undefined) prefix.push(flag!, value);
    }
    const values = new Map<string, unknown>();
    call.argv.forEach((arg, index) => { if (arg instanceof Payload) values.set(String(index), arg.value); });
    if (call.policies !== undefined) values.set('policies', call.policies);
    const limit = Math.min(config.inputLimit, call.payloadLimit ?? config.inputLimit);
    if (!Number.isSafeInteger(limit) || limit <= 0) throw new TransportError('invalid_input');
    let started = false;
    try {
      return await payloadFiles(values, limit, async paths => {
        if (call.policies !== undefined) prefix.push('--policies', paths.get('policies')!);
        const argv = call.argv.map((arg, index) => arg instanceof Payload ? paths.get(String(index))! : arg);
        try {
          const result = await invoke(config.executable, [...prefix, ...argv], { ...options, captureLimit: config.captureLimit });
          started = true;
          return result;
        } catch (error) {
          if (error instanceof TransportError) started = error.started;
          throw error;
        }
      });
    } catch (error) {
      if (error instanceof TransportError) { error.indeterminate = !!call.mutates && error.started; throw error; }
      throw new TransportError('payload_io', started, started && !!call.mutates);
    }
  }

  async json(config: Config, call: Call): Promise<unknown> {
    const result = await this.run(config, call, { capture: true, timeout: config.timeout });
    try {
      if (result.exitCode !== 0) {
        const envelope = decode(result.stderr ?? new Uint8Array());
        const error = object(envelope) ? envelope.error : undefined;
        if (object(error) && typeof error.code === 'string' && typeof error.message === 'string' && object(error.details)
          && Object.values(error.details).every(value => typeof value === 'string')) {
          throw new SaucepanError(error.code, error.message, error.details as Record<string, string>);
        }
        throw new TransportError('protocol', true);
      }
      const value = decode(result.stdout ?? new Uint8Array());
      checkShape(value, call);
      return value;
    } catch (error) {
      if (error instanceof TransportError) error.indeterminate = !!call.mutates;
      throw error;
    }
  }
}
