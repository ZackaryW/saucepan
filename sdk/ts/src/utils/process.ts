import { spawn } from 'node:child_process';
import type { RunOptions, RunResult } from '../contracts.js';
import { TransportError } from '../errors.js';

export interface InvocationOptions extends RunOptions {
  readonly captureLimit?: number;
  readonly cwd?: string;
  readonly env?: NodeJS.ProcessEnv;
}

export function invoke(executable: string, argv: readonly string[], options: InvocationOptions = {}): Promise<RunResult> {
  const cap = options.captureLimit ?? 16 * 1024 * 1024;
  const timeout = options.timeout;
  if (!Number.isSafeInteger(cap) || cap <= 0 || (timeout != null && (!Number.isFinite(timeout) || timeout <= 0))) return Promise.reject(new TransportError('invalid_input'));
  if (options.signal?.aborted) return Promise.reject(new TransportError('cancelled'));
  if (process.platform === 'win32' && /\.(bat|cmd)$/i.test(executable)) return Promise.reject(new TransportError('invalid_executable'));
  return new Promise((resolve, reject) => {
    let child;
    try {
      child = spawn(executable, [...argv], { shell: false, windowsHide: true, cwd: options.cwd, env: options.env,
        stdio: ['inherit', options.capture ? 'pipe' : 'inherit', options.capture ? 'pipe' : 'inherit'] });
    } catch {
      reject(new TransportError('spawn'));
      return;
    }
    let started = false;
    let reason: string | null = null;
    let settled = false;
    const buffers: Buffer[][] = [[], []];
    const counts = [0, 0];
    const finish = (code: number | null, signal: NodeJS.Signals | null) => {
      if (settled) return;
      settled = true;
      if (timer) clearTimeout(timer);
      options.signal?.removeEventListener('abort', abort);
      if (reason) {
        // Only SDK-owned pipe ends; native retention descriptors are untouched.
        child.stdout?.destroy(); child.stderr?.destroy();
        reject(new TransportError(reason, started));
      } else {
        resolve({ exitCode: code, signal, stdout: options.capture ? Buffer.concat(buffers[0]) : null,
          stderr: options.capture ? Buffer.concat(buffers[1]) : null });
      }
    };
    const stop = (kind: string) => {
      reason ??= kind;
      if (child.exitCode !== null || child.signalCode !== null) finish(child.exitCode, child.signalCode);
      else child.kill('SIGKILL');
    };
    const abort = () => stop('cancelled');
    const timer = timeout == null ? undefined : setTimeout(() => stop('timeout'), timeout);
    options.signal?.addEventListener('abort', abort, { once: true });
    child.once('spawn', () => { started = true; if (options.signal?.aborted) abort(); });
    child.once('error', (error: NodeJS.ErrnoException) => {
      reason = error.code === 'ENOENT' ? 'missing_executable' : 'spawn';
      finish(null, null);
    });
    [child.stdout, child.stderr].forEach((stream, index) => stream?.on('data', (chunk: Buffer) => {
      const room = cap - counts[index];
      buffers[index].push(chunk.subarray(0, Math.max(0, room)));
      counts[index] += Math.min(chunk.length, room);
      if (chunk.length > room) stop('output_limit');
    }));
    child.once('exit', (code, signal) => { if (reason) finish(code, signal); });
    child.once('close', finish);
  });
}
