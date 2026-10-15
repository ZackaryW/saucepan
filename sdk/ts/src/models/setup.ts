/** Wire data is snake_case; the Rust manager owns all recipes and evaluation. */
import type { Policy } from './policy.js';
import type { Evidence } from './source.js';
import type { Retrieval, Acquisition, SourceInput } from './composition.js';
export type ProfileName = 'managed' | 'explicit-update' | 'temporary';
export interface ProfileExpansion {
  name: ProfileName; for_install: boolean; digest: string; catalog: Policy[];
  retrieval_policy_refs: string[]; acquisition_policy_refs: string[]; combined_policy_refs: string[];
}
export type ExplainRequest =
  | { operation: 'retrieve'; request: Retrieval; scope?: string | null }
  | { operation: 'acquire'; source: SourceInput; request: Acquisition; scope?: string | null }
  | { operation: 'compose'; retrieval: Retrieval; acquisition: Acquisition; scope?: string | null }
  | { operation: 'claim'; app: string; claim: string };
export type Origin = 'built_in' | 'source' | 'application' | 'request' | 'adopted_claim';
export interface AxisExplanation {
  applicable: boolean; value: unknown; origin: Origin | null; evidence: { uuid: string; digest: string } | null;
}
export interface PolicyExplanation {
  axes: Record<string, AxisExplanation>;
  restrictions: { origin: Origin; record: Policy; digest: string }[];
}
export interface Probe { args: string[]; stdout_contains: string; timeout_seconds: number; }
export type Phase = 'preparing' | 'commit_intent' | 'cleanup_pending' | 'complete' | 'failed' | 'conflict';
export interface Operation {
  operation: string; app: string; action: Evidence; fingerprint: string; status: Phase; changed: boolean;
  expected_revision: number | null; claim: string; candidate: string | null;
  desired: Evidence | null; desired_registry: Evidence | null; source_policy: Evidence | null;
  shims: Evidence[]; pending: string[]; reason: Evidence | null; leftovers: string[];
}
/** Exit 2 preserves a noncomplete receipt instead of triggering an automatic retry. */
export interface LifecycleResult<T> { exit_code: 0 | 2; result: T; }
