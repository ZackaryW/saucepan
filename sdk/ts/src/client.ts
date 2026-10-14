import type { Call, Config, RunOptions, RunResult, Transport } from './contracts.js';
import { TransportError } from './errors.js';
import { Payload } from './contracts.js';
import type * as c from './models/claim.js';
import type * as co from './models/composition.js';
import type * as g from './models/group.js';
import type * as p from './models/pin.js';
import type * as po from './models/policy.js';
import type * as s from './models/source.js';

function flags(values: Record<string, string | number | readonly string[] | Payload | undefined>): (string | Payload)[] {
  const argv: (string | Payload)[] = [];
  for (const [name, value] of Object.entries(values)) {
    if (value === undefined) continue;
    if (typeof value === 'number' && !Number.isSafeInteger(value)) throw new TransportError('invalid_input');
    const flag = '--' + name.replaceAll('_', '-');
    for (const item of Array.isArray(value) ? value : [value]) argv.push(flag, item instanceof Payload ? item : String(item));
  }
  return argv;
}

class DefaultTransport implements Transport {
  async json(config: Config, call: Call): Promise<unknown> {
    const { NodeTransport } = await import('./transport.js');
    return new NodeTransport().json(config, call);
  }
  async run(config: Config, call: Call, options: RunOptions): Promise<RunResult> {
    const { NodeTransport } = await import('./transport.js');
    return new NodeTransport().run(config, call, options);
  }
}

export interface ClientOptions {
  executable?: string;
  root?: string;
  credentialFile?: string;
  /** JSON operation deadline in milliseconds; null explicitly disables it. */
  timeout?: number | null;
  captureLimit?: number;
  inputLimit?: number;
  transport?: Transport;
}

export class Client {
  private config: Config;
  private readonly transport: Transport;
  constructor(options: ClientOptions = {}) {
    this.config = Object.freeze({ executable: options.executable ?? 'saucepan', root: options.root,
      credentialFile: options.credentialFile, timeout: options.timeout === undefined ? 120_000 : options.timeout,
      captureLimit: options.captureLimit ?? 16 * 1024 * 1024, inputLimit: options.inputLimit ?? 16 * 1024 * 1024 });
    this.transport = options.transport ?? new DefaultTransport();
  }
  forApp(app: string, options: { credentialFile?: string } = {}): Client {
    const client = new Client({ transport: this.transport });
    client.config = Object.freeze({ ...this.config, app, credentialFile: options.credentialFile });
    return client;
  }
  private identity(app: string): string {
    if (this.config.app !== undefined && this.config.app !== app) throw new TransportError('context_mismatch');
    return app;
  }
  private async json<T>(argv: readonly (string | Payload)[], call: Omit<Call, 'argv'> = {}): Promise<T> {
    return await this.transport.json(this.config, { ...call, argv }) as T;
  }
  private execute(argv: readonly (string | Payload)[], command: readonly string[], options: RunOptions = {}, call: Omit<Call, 'argv'> = {}): Promise<RunResult> {
    if (!Array.isArray(command) || command.length === 0) throw new TransportError('invalid_input');
    return this.transport.run(this.config, { ...call, argv: [...argv, '--', ...command] }, options);
  }
  querySources(query?: s.SourceQuery): Promise<s.SourceQueryResult> {
    return this.json(['query-sources', ...(query === undefined ? [] : [new Payload(query)])], { required: ['generation', 'sources'] });
  }
}
