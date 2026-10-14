/** Current Rust wire contract; policies are evaluated only by the manager. */
import type { Evidence, Platform, Selector } from "./source.js";
export interface Selection {
  platform?: Platform;
  filter_refs?: (string)[];
  format?: "natural" | "raw" | "zip";
  filename?: string;
  folder?: string;
  bin?: (string)[];
  sha256?: string;
}
export interface AcquireRequest {
  source_uuid: string;
  selector: Selector;
  selection?: Selection;
  policy_refs?: (string)[];
}
export interface EnsureRequest extends AcquireRequest {
  scope: string;
  context?: Record<string, string>;
}
export interface ApplicationRegistration {
  registration_key: string;
  name: string;
  policy_refs?: (string)[];
  credentials_required?: boolean;
}
export interface ApplicationConfiguration {
  expected_revision: number;
  policy_refs: (string)[];
  credential?: "enable" | "rotate" | "disable" | null;
}
export interface Application {
  uuid: string;
  name: string;
  revision: number;
  state: string;
  settings: Evidence;
  active: Evidence | null;
  shims: (Evidence)[];
}
export interface Claim {
  uuid: string;
  scope: string;
  entry_uuid: string;
  context: Record<string, string>;
  policy_refs: (string)[];
  request: Evidence;
  effective: Evidence;
}
export interface ClaimView {
  claim: Claim;
  state: "ready" | "reclaim_pending" | "reclaimed";
}
export interface CoveredView {
  app: string;
  claim: string;
  entry: string;
  relative: string | null;
}
export interface OutputEntry {
  uuid: string;
  state: "ready" | "reclaim_pending" | "reclaimed";
  request: Evidence;
  resolved: Evidence;
  content_digest: string;
  sequence: number;
  files: Record<string, Evidence>;
}
export interface ReclaimReport {
  effective_keep: Record<string, number>;
  reclaimed: (string)[];
  deferred: Record<string, string>;
  pending: Record<string, string>;
}
export interface Acquired {
  entry: OutputEntry;
  directory: string;
  reused: boolean;
  update_checked: boolean;
  stale: boolean;
  reclaim: ReclaimReport;
}
export interface Claimed {
  claim: Claim;
  view: CoveredView;
  acquired: Acquired;
}
export interface ClaimAdoption {
  app: string;
  revision: number;
  claim: Claim;
}
export interface Condition {
  key: string;
  op: "eq" | "glob";
  value: string;
}
export interface ClaimQuery {
  "app": string;
  "scope"?: string;
  "where": (Condition)[];
}
export interface DiscoveryQuery {
  "scope"?: string;
  "where": (Condition)[];
}
export interface QueryResult {
  app: string;
  revision: number;
  claims: (ClaimView)[];
}
export interface ReleaseResult {
  released: string;
}
