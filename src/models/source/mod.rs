use super::policy::Refresh;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    Local {
        path: PathBuf,
    },
    Url {
        url: String,
    },
    GithubRelease {
        repo: String,
    },
    /// Cloned with the token borrowed from `gh`.
    GithubRepo {
        repo: String,
    },
    /// One file at a commit, downloaded from GitHub's raw endpoint.
    GithubRaw {
        repo: String,
        path: String,
    },
    /// Source snapshot ZIP of one commit, without history.
    GithubSource {
        repo: String,
    },
    /// Plain Git from any host; never consults `gh`.
    GitRepo {
        origin: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token_file: Option<String>,
    },
    /// One file through a GitLab or Gitea raw-file route.
    GitRaw {
        host: String,
        platform: GitPlatform,
        repo: String,
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token_file: Option<String>,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitPlatform {
    Gitlab,
    Gitea,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRecord {
    pub policy_refs: super::policy::PolicyRefs,
    pub policy_revision: u64,
    pub uuid: Uuid,
    pub source: Source,
    /// Explicit anonymous catalogue visibility; independent of claim visibility.
    #[serde(skip)]
    pub public_discovery: bool,
    #[serde(skip)]
    pub refresh: Refresh,
    /// Derived from the source's immutable policy definitions.
    #[serde(skip)]
    pub retrieval_freshness: Option<super::policy::Freshness>,
    pub discovery: BTreeMap<String, Discovery>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Discovery {
    pub checked_at: u64,
    pub resolved: Resolved,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub id: u64,
    pub name: String,
    pub url: String,
    pub digest: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Resolved {
    Local,
    Git {
        commit: String,
    },
    Release {
        id: u64,
        tag: String,
        assets: Vec<Asset>,
    },
    Url {
        url: String,
        etag: Option<String>,
        last_modified: Option<String>,
    },
}
