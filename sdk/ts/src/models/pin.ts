/** Current Rust wire contract; policies are evaluated only by the manager. */
import type { GroupDescriptor, GroupRecord } from "./group.js";
import type { Evidence } from "./source.js";
export interface GroupPinRequest {
  registration_key: string;
  policy_refs: (string)[];
}
export interface PinRequest extends GroupPinRequest {
  outputs: (string)[];
}
export interface GroupPinReceipt {
  operation: "prepare" | "pin";
  expected_revision: number;
  group: GroupRecord;
  stale: (string)[];
}
export interface PinRecord {
  uuid: string;
  owner: string | null;
  request: PinRequest;
  witnesses: (Evidence)[];
  state: "active" | "released";
  group: GroupPinReceipt | null;
}
export interface PinOutput {
  path: string;
  state: "ready" | "reclaim_pending" | "reclaimed";
  available: boolean;
}
export interface PinInspection extends PinRecord {
  outputs: Record<string, PinOutput>;
}
export interface PinnedOutputs {
  pin: PinRecord;
  paths: Record<string, string>;
}
export interface PreparedGroup {
  group: GroupRecord;
  pin: PinRecord | null;
  resources: GroupDescriptor;
  stale: (string)[];
}
