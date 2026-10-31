/** Current Rust wire contract; policies are evaluated only by the manager. */
export type Json = null | boolean | number | number | string | (Json)[] | { [key: string]: Json };
export type Evidence = { [key: string]: Json };
export interface LocalSource {
  type: "local";
  path: string;
}
export interface UrlSource {
  type: "url";
  url: string;
}
export interface GitRepoSource {
  type: "git_repo";
  origin: string;
  token_file?: string;
}
export interface GithubSource {
  type: "github_repo" | "github_release" | "github_source";
  repo: string;
}
export interface GithubRawSource {
  type: "github_raw";
  repo: string;
  path: string;
}
export interface GitRawSource {
  type: "git_raw";
  host: string;
  platform: "gitlab" | "gitea";
  repo: string;
  path: string;
  token_file?: string;
}
export type Source = LocalSource | UrlSource | GitRepoSource | GithubSource | GithubRawSource | GitRawSource;
export interface SimpleSelector {
  type: "local" | "url" | "latest_stable";
}
export interface TagSelector {
  type: "tag";
  tag: string;
}
export interface RefSelector {
  type: "ref";
  reference: string;
}
export interface CommitSelector {
  type: "commit";
  commit: string;
}
export type Selector = SimpleSelector | TagSelector | RefSelector | CommitSelector;
export interface Platform {
  os: string;
  arch: string;
}
export interface SourceQuery {
  uuid?: string;
  type?: "local" | "url" | "git_repo" | "github_repo" | "github_release" | "github_raw" | "git_raw" | "github_source";
  repo?: string;
  origin?: string;
  url?: string;
}
export interface SourceView {
  uuid: string;
  source: Source;
  public_discovery: boolean;
}
export interface SourceQueryResult {
  generation: number;
  sources: (SourceView)[];
}
export interface SourceRecord {
  uuid: string;
  source: Source;
  policy_revision: number;
  policy_refs: (string)[];
  discovery: Record<string, Evidence>;
}
export interface InitResult {
  root: string;
  integrity: "basic" | "keyring";
}
export interface IndexRecord {
  type: "application" | "output" | "retrieval" | "acquisition";
  uuid: string;
  revision?: number;
}
