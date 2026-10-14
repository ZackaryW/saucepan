/** Current Rust wire contract; policies are evaluated only by the manager. */
import type { Evidence, Platform, Selector, Source } from "./source.js";
export interface InlineSource {
  kind: "inline";
  source: Source;
}
export interface RegisteredSource {
  kind: "registered";
  source_uuid: string;
}
export type SourceInput = InlineSource | RegisteredSource;
export interface AssetSelection {
  platform?: Platform;
  filter_refs?: (string)[];
}
export interface Retrieval {
  source: SourceInput;
  selector: Selector;
  asset?: AssetSelection;
  policy_refs?: (string)[];
  destination?: string;
  expected_sha256?: string;
}
export interface SimpleDestination {
  mode: "managed" | "temporary";
}
export interface DirectoryDestination {
  mode: "directory";
  path: string;
}
export interface Acquisition {
  destination?: SimpleDestination | DirectoryDestination;
  format?: "natural" | "raw" | "zip";
  filename?: string;
  folder?: string;
  bin?: (string)[];
  policy_refs?: (string)[];
}
export interface CompositionRequest {
  retrieval: Retrieval;
  acquisition: Acquisition;
}
export interface ManagedMaterial {
  mode: "managed";
  uuid: string;
}
export interface Ownership {
  kind: "managed" | "temporary" | "caller";
  uuid?: string;
}
export interface RetrievedDescriptor {
  ownership: Ownership;
  path: string;
  kind: Evidence;
  source: SourceInput;
  resolved: Evidence;
  dependencies: Record<string, string>;
  verification: Evidence;
  checked_at: number | null;
  origin_checked: boolean;
  reused: boolean;
  stale: boolean;
}
export interface RetrievalReceipt {
  request: Retrieval;
  descriptor: RetrievedDescriptor;
}
export interface CallerMaterial extends RetrievalReceipt {
  mode: "caller";
}
export interface AcquireRetrievedRequest {
  retrieval: ManagedMaterial | CallerMaterial;
  acquisition: Acquisition;
}
export interface AcquiredDescriptor {
  ownership: Ownership;
  path: string;
  selected_source: Evidence;
  checked_source: Evidence | null;
  verification: Evidence;
  reused: boolean;
  changed: boolean;
  stale: boolean;
  update_policy: Evidence;
  revision_policy: Evidence | null;
  reclaim: Evidence | null;
}
export interface ResolvedResource {
  entry_uuid: string;
  directory: string;
  verification: "metadata" | "full";
}
export interface SourcePath {
  path: string;
}
export interface PurgeResult {
  purged: boolean;
}
