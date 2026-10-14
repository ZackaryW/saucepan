/** Current Rust wire contract; policies are evaluated only by the manager. */
import type { Platform } from "./source.js";
export type Action = "read" | "discover" | "acquire" | "update" | "release" | "recover" | "lease" | "pin" | "unpin" | "activate" | "remove" | "set_context" | "register_policy";
export interface Base {
  uuid: string;
}
export interface AccessPolicy extends Base {
  type: "access_policy";
  sources?: (string)[];
  scopes?: (string)[];
  actions?: (Action)[];
}
export interface SourceVisibilityPolicy extends Base {
  type: "source_visibility_policy";
  public_discovery: boolean;
}
export interface ClaimVisibilityPolicy extends Base {
  type: "claim_visibility_policy";
  public_scopes: (string)[];
}
export interface CachePolicy extends Base {
  type: "cache_policy";
  mode: "cached" | "temporary" | "no_store";
}
export interface ImmediateRefreshPolicy extends Base {
  type: "refresh_policy";
  mode: "always" | "cache_only";
}
export interface TimedPolicy extends Base {
  type: "refresh_policy" | "update_policy";
  mode: "max_age";
  seconds: number;
}
export interface UpdatePolicy extends Base {
  type: "update_policy";
  mode: "manual" | "always";
}
export interface StalePolicy extends Base {
  type: "stale_policy";
  allow_stale: boolean;
}
export interface PayloadVerificationPolicy extends Base {
  type: "verification_policy";
  policy_target: "payload";
  mode: "none" | "if_available" | "required";
}
export interface PathVerificationPolicy extends Base {
  type: "verification_policy";
  policy_target: "path";
  mode: "metadata" | "full";
}
export interface AssetFilterPolicy extends Base {
  type: "github_release_asset_filter";
  platform?: Platform | null;
  asset_patterns: (string)[];
}
export interface SelectionPolicy extends Base {
  type: "selection_policy";
  policy_target: "github_release_asset_filter";
  strategy: "first_match_win";
}
export interface IdenticalPullPolicy extends Base {
  type: "identical_pull_policy";
  strategy: "reuse" | "new_entry";
}
export interface NoHistoryPolicy extends Base {
  type: "revision_policy";
  history: "none";
}
export interface LatestHistoryPolicy extends Base {
  type: "revision_policy";
  history: "latest";
  keep: number;
}
export interface ReclaimPolicy extends Base {
  type: "reclaim_policy";
  strategy: "report_unavailable" | "restore_exact" | "refresh";
}
export interface UseableAppPolicy extends Base {
  type: "useable_app_policy";
}
export interface PinPolicy extends Base {
  type: "pin_policy";
  retention: "until_released";
}
export interface PolicyConstraints extends Base {
  type: "policy_constraints";
  minimum_keep?: number;
  maximum_keep?: number;
  allowed_identical?: ("reuse" | "new_entry")[];
  allowed_reclaim?: ("report_unavailable" | "restore_exact" | "refresh")[];
  allow_useable_app?: boolean;
  require_useable_app?: boolean;
  allow_automatic_updates?: boolean;
  minimum_update_interval?: number;
}
export type Policy = AccessPolicy | SourceVisibilityPolicy | ClaimVisibilityPolicy | CachePolicy | ImmediateRefreshPolicy | TimedPolicy | UpdatePolicy | StalePolicy | PayloadVerificationPolicy | PathVerificationPolicy | AssetFilterPolicy | SelectionPolicy | IdenticalPullPolicy | NoHistoryPolicy | LatestHistoryPolicy | ReclaimPolicy | UseableAppPolicy | PinPolicy | PolicyConstraints;
export interface ApplicationAdoption {
  app: string;
  revision: number;
  policy_refs: (string)[];
}
