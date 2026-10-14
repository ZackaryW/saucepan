/** Current Rust wire contract; policies are evaluated only by the manager. */
import type { AcquireRequest } from "./claim.js";
import type { Evidence } from "./source.js";
export interface ExistingTarget {
  type: "output" | "claim";
  uuid: string;
}
export interface ClaimIntent {
  scope: string;
  context?: Record<string, string>;
}
export interface AcquireTarget extends AcquireRequest {
  type: "acquire";
  claim?: ClaimIntent;
}
export interface Member {
  name: string;
  target: ExistingTarget | AcquireTarget;
  relative?: string;
}
export interface InlineMember {
  name: string;
  target: ExistingTarget;
  relative?: string;
}
export interface GroupDefinition {
  registration_key: string;
  members: (Member)[];
  policy_refs?: (string)[];
}
export interface GroupRequest {
  members: (InlineMember)[];
  policy_refs?: (string)[];
}
export interface SelectedMember {
  output_uuid: string;
  claim_uuid: string | null;
  scope: string | null;
  relative: string | null;
  selected_source: Evidence;
  checked_source: Evidence | null;
  last_success: number | null;
  policy_provenance: (Evidence)[];
}
export interface Generation {
  uuid: string;
  members: Record<string, SelectedMember>;
}
export interface GroupRecord {
  uuid: string;
  owner: string | null;
  revision: number;
  definition: GroupDefinition;
  witnesses: Record<string, Evidence>;
  selected: Generation | null;
}
export interface GroupMemberDescriptor {
  output_uuid: string;
  claim_uuid?: string;
  directory: string;
  relative: string | null;
  verification: "metadata" | "full";
  policy_provenance: (Evidence)[];
}
export interface GroupDescriptor {
  members: Record<string, GroupMemberDescriptor>;
}
export interface RemoveResult {
  removed: string;
}
