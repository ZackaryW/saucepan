/** JSON shapes of the Saucepan 0.5 central-store protocol. */
export interface AppSettings {
  retain_snapshots: boolean;
  verify_content: boolean;
  allow_local_fallback: boolean;
}
export interface Filters { source_ids: string[]; providers: Array<'git' | 'url' | 'local'> }
export interface Configuration { settings?: AppSettings; filters?: Filters }
export interface AppToken { version: 1; app: string; token: number[] }
export type Source =
  | { provider: 'git'; origin: string; reference: string }
  | { provider: 'url'; url: string; download: { format: 'file'; name: string } | { format: 'zip' } }
  | { provider: 'local'; path: string };
export interface Recipe { source: Source; folder?: string | null; commit?: string | null }
export interface FileRecord { digest: string | null; executable: boolean }
export interface Artifact {
  id: string; source_id: string; source: Source; snapshot_id: string;
  revision: string; folder: string | null; content_id: string;
  files: Record<string, FileRecord>;
}
export interface Acquired {
  artifact: Artifact; directory: string; fallback: boolean;
  update_checked: boolean; content_verified: boolean;
}
export interface AppView {
  version: 1; app: string; settings: AppSettings; filters: Filters;
  entries: Record<string, Artifact>;
}
export interface Snapshot {
  id: string; revision: string; content_id: string;
  files: Record<string, FileRecord>; dependencies: Record<string, string>;
  zip_digest: string | null; last_used: number; created: number;
}
export interface SourceState {
  source: Source; current: Snapshot | null; history: Snapshot[]; sequence: number;
}
export interface ClientOptions {
  /** Defaults to the user-level .saucepan/bin executable. No downloader is used. */
  binary?: string;
  app?: string;
  /** A caller-owned marker file. Never rewritten by this client. */
  marker?: string;
  /** Alternative to marker: serialized privately for each call. */
  token?: AppToken;
  authoritative?: boolean;
  /** Explicit isolated test mode. Both fields are required; key is 64 hex characters. */
  testStore?: { root: string; key: string };
  /** Zero means no timeout (default). */
  timeoutMs?: number;
  /** Maximum bytes per output stream; defaults to 16 MiB. */
  maxBuffer?: number;
}
