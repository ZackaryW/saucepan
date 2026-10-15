/** Transport contracts have no Node runtime imports. */
export interface Config {
  readonly executable: string;
  readonly root?: string | undefined;
  readonly app?: string | undefined;
  readonly credentialFile?: string | undefined;
  readonly timeout: number | null;
  readonly captureLimit: number;
  readonly inputLimit: number;
}
export class Payload {
  constructor(readonly value: unknown) {}
}
export interface Call {
  readonly argv: readonly (string | Payload)[];
  readonly mutates?: boolean | undefined;
  readonly shape?: 'object' | 'array' | undefined;
  readonly required?: readonly string[] | undefined;
  readonly payloadLimit?: number | undefined;
  readonly policies?: unknown;
  readonly lifecycle?: 'operation' | 'reconcile' | 'inspection';
}
export interface RunOptions {
  readonly capture?: boolean;
  /** Milliseconds. Omitted/null means no execution deadline. */
  readonly timeout?: number | null;
  readonly signal?: AbortSignal;
}
export interface RunResult {
  readonly exitCode: number | null;
  readonly signal: string | null;
  readonly stdout: Uint8Array | null;
  readonly stderr: Uint8Array | null;
}
export interface Transport {
  json(config: Config, call: Call): Promise<unknown>;
  run(config: Config, call: Call, options: RunOptions): Promise<RunResult>;
}
