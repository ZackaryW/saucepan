import { execFile } from 'node:child_process';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { homedir, tmpdir } from 'node:os';
import { join } from 'node:path';
import type { Acquired, AppToken, AppView, ClientOptions, Configuration, Recipe, SourceState } from './types.js';
export type * from './types.js';

export class SaucepanError extends Error {
  constructor(
    message: string,
    readonly exitCode: number | null,
    readonly stdout: string,
    readonly stderr: string,
    readonly code: string | undefined = undefined,
    readonly signal: string | null = null,
    readonly killed = false,
  ) { super(message); this.name = 'SaucepanError'; }
}

export function sharedExecutablePath(): string {
  return join(homedir(), '.saucepan', 'bin', process.platform === 'win32' ? 'saucepan.exe' : 'saucepan');
}

/** Asynchronous, shell-free adapter. All index and policy decisions stay in the CLI. */
export class Saucepan {
  #options: ClientOptions;
  constructor(options: ClientOptions = {}) {
    if (options.marker !== undefined && options.token !== undefined) {
      throw new TypeError('Choose a marker file or a token, not both');
    }
    if (options.testStore && (!options.testStore.root || !/^[0-9a-f]{64}$/i.test(options.testStore.key ?? ''))) {
      throw new TypeError('A test store requires a root and a 64-character hexadecimal key');
    }
    for (const [name, value, minimum] of [
      ['timeoutMs', options.timeoutMs, 0], ['maxBuffer', options.maxBuffer, 1],
    ] as const) {
      if (value !== undefined && (!Number.isSafeInteger(value) || value < minimum)) {
        throw new TypeError(`${name} must be an integer >= ${minimum}`);
      }
    }
    this.#options = structuredClone(options);
  }

  /** Create an independent client context; registration and settings remain central. */
  forApp(app: string | AppToken): Saucepan {
    const { app: _app, token: _token, marker: _marker, authoritative: _auth, ...base } = this.#options;
    return new Saucepan(typeof app === 'string' ? { ...base, app } : { ...base, token: app });
  }

  init(): Promise<{ created: true }> { return this.#call('init'); }
  register(app: string, configuration: Configuration = {}): Promise<AppToken> {
    return this.#call<AppToken>('register', [app], configuration).then(value => this.#versioned(value));
  }
  configure(configuration: Configuration): Promise<{ configured: true }> {
    return this.#call('configure', [], configuration);
  }
  acquire(recipe: Recipe): Promise<Acquired> { return this.#call('acquire', [], {}, recipe); }
  view(): Promise<AppView> { return this.#call<AppView>('view').then(value => this.#versioned(value)); }
  verify(view: AppView): Promise<{ verified: true }> { return this.#call('verify', [], {}, view); }
  path(artifact: string): Promise<string | null> { return this.#call('path', [artifact]); }
  mirror(artifact: string, destination: string): Promise<{ directory: string }> {
    return this.#call('mirror', [artifact, destination]);
  }
  history(source: string): Promise<SourceState | null> { return this.#call('history', [source]); }
  snapshot(source: string, snapshot: string, folder?: string): Promise<Acquired> {
    return this.#call('snapshot', [source, snapshot], {}, undefined, folder);
  }
  sharedExecutable(): Promise<string> { return this.#call('shared-executable'); }

  #versioned<T extends { version: number }>(value: T): T {
    if (value?.version !== 1) throw new SaucepanError('Unsupported response format version', null, '', '', 'PROTOCOL_VERSION');
    return value;
  }

  async #call<T>(command: string, positionals: string[] = [], configuration: Configuration = {}, document?: unknown, folder?: string): Promise<T> {
    let temporary: string | undefined;
    let sequence = 0;
    const jsonFile = async (value: unknown): Promise<string> => {
      temporary ??= await mkdtemp(join(tmpdir(), 'saucepan-request-'));
      const path = join(temporary, `${sequence++}.json`);
      await writeFile(path, JSON.stringify(value), { mode: 0o600, flag: 'wx' });
      return path;
    };
    try {
      const args: string[] = [];
      const options = this.#options;
      if (options.testStore) args.push(`--test-root=${options.testStore.root}`, `--test-key=${options.testStore.key}`);
      if (options.app !== undefined) args.push(`--app=${options.app}`);
      if (options.marker !== undefined) args.push(`--marker=${options.marker}`);
      if (options.token !== undefined) args.push(`--marker=${await jsonFile(options.token)}`);
      if (options.authoritative) args.push('--authoritative');
      args.push(command);
      if (configuration.settings !== undefined) args.push('--settings', await jsonFile(configuration.settings));
      if (configuration.filters !== undefined) args.push('--filters', await jsonFile(configuration.filters));
      if (folder !== undefined) args.push(`--folder=${folder}`);
      const inputs = document === undefined ? positionals : [await jsonFile(document), ...positionals];
      if (inputs.length) args.push('--', ...inputs);
      const stdout = await this.#execute(args);
      try { return JSON.parse(stdout) as T; }
      catch { throw new SaucepanError('Saucepan returned invalid JSON', 0, stdout, '', 'INVALID_JSON'); }
    } finally {
      if (temporary) await rm(temporary, { recursive: true, force: true });
    }
  }

  #execute(args: string[]): Promise<string> {
    return new Promise((resolve, reject) => {
      execFile(this.#options.binary ?? sharedExecutablePath(), args, {
        encoding: 'utf8', shell: false, windowsHide: true,
        timeout: this.#options.timeoutMs ?? 0, maxBuffer: this.#options.maxBuffer ?? 16 * 1024 * 1024,
      }, (error, stdout, stderr) => {
        if (!error) { resolve(stdout); return; }
        const exitCode = typeof error.code === 'number' ? error.code : null;
        const code = typeof error.code === 'string' ? error.code : undefined;
        // Node's default error message includes the command and test key. Do not expose it.
        reject(new SaucepanError(
          exitCode === null ? `Could not execute Saucepan (${code ?? error.signal ?? 'terminated'})` : `Saucepan exited with code ${exitCode}: ${stderr.trim()}`,
          exitCode, stdout, stderr, code, error.signal ?? null, error.killed ?? false,
        ));
      });
    });
  }
}
