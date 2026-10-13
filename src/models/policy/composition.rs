//! Independent policies for source retrieval, copied-result updates and history.
use super::Eviction;
use crate::error::{Error, ErrorCode, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Freshness {
    MaxAge { seconds: u64 },
    Always {},
    CacheOnly {},
}
impl Default for Freshness {
    fn default() -> Self {
        Self::MaxAge { seconds: 86_400 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum UpdatePolicy {
    Manual {},
    Always {},
    MaxAge { seconds: u64 },
}
impl Default for UpdatePolicy {
    fn default() -> Self {
        Self::Manual {}
    }
}
impl UpdatePolicy {
    /// Future timestamps cannot prove freshness. Checks never extend their own TTL.
    pub fn due(self, last_success: Option<u64>, now: u64) -> bool {
        match self {
            Self::Manual {} => false,
            Self::Always {} => true,
            Self::MaxAge { seconds } => last_success
                .and_then(|last| now.checked_sub(last))
                .is_none_or(|elapsed| elapsed >= seconds),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "history", rename_all = "snake_case", deny_unknown_fields)]
pub enum RevisionPolicy {
    None {},
    Latest { keep: usize },
}
impl Default for RevisionPolicy {
    fn default() -> Self {
        Self::Latest { keep: 10 }
    }
}
impl RevisionPolicy {
    pub fn keep(self) -> Result<usize> {
        match self {
            Self::None {} => Ok(1),
            Self::Latest { keep: 0 } => Err(Error::new(
                ErrorCode::InvalidRequest,
                "revision retention must be positive",
            )),
            Self::Latest { keep } => Ok(keep),
        }
    }
}
// Both public spellings feed the existing retention evaluator. No second window.
impl TryFrom<RevisionPolicy> for Eviction {
    type Error = Error;
    fn try_from(value: RevisionPolicy) -> Result<Self> {
        Ok(Self::Latest {
            keep: value.keep()?,
        })
    }
}
impl TryFrom<&Eviction> for RevisionPolicy {
    type Error = Error;
    fn try_from(value: &Eviction) -> Result<Self> {
        let Eviction::Latest { keep } = *value;
        let policy = if keep == 1 {
            Self::None {}
        } else {
            Self::Latest { keep }
        };
        policy.keep()?;
        Ok(policy)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadVerification {
    None,
    #[default]
    IfAvailable,
    Required,
}
