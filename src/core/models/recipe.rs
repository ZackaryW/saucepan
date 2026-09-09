use crate::utils::{hash::sha256, json::sorted_json, path::native_relative_path};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "provider", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    Git { origin: String, reference: String },
    Url { url: String, download: Download },
    Local { path: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "format", rename_all = "snake_case", deny_unknown_fields)]
pub enum Download {
    File { name: String },
    Zip,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub source: Source,
    #[serde(default)]
    pub folder: Option<String>,
    /// Exact Git commit to read without advancing a tracked source.
    #[serde(default)]
    pub commit: Option<String>,
}

impl Source {
    pub fn canonical(&self) -> Result<Self> {
        Ok(match self {
            Self::Git { origin, reference } => {
                ensure!(
                    !reference.is_empty()
                        && !reference.starts_with('-')
                        && !reference.contains("..")
                        && !reference.contains("@{")
                        && !reference
                            .chars()
                            .any(|c| c.is_control() || c.is_whitespace() || "~^:?*[\\".contains(c)),
                    "invalid Git reference"
                );
                let origin = if let Ok(url) = url::Url::parse(origin) {
                    if url.scheme().len() == 1 {
                        url::Url::from_file_path(std::fs::canonicalize(origin)?)
                            .map_err(|_| anyhow::anyhow!("invalid Git path"))?
                    } else {
                        url
                    }
                } else if let Some((host, path)) = origin
                    .split_once(':')
                    .filter(|(host, _)| !host.contains('/') && host.contains('@'))
                {
                    url::Url::parse(&format!("ssh://{host}/{path}"))?
                } else {
                    url::Url::from_file_path(std::fs::canonicalize(origin)?)
                        .map_err(|_| anyhow::anyhow!("invalid Git path"))?
                };
                ensure!(
                    matches!(origin.scheme(), "https" | "http" | "ssh" | "git" | "file"),
                    "unsupported Git transport"
                );
                ensure!(
                    origin.fragment().is_none() && origin.query().is_none(),
                    "Git origin must not have a query or fragment"
                );
                Self::Git {
                    origin: origin.as_str().trim_end_matches('/').to_owned(),
                    reference: reference.clone(),
                }
            }
            Self::Url { url, download } => {
                let parsed = url::Url::parse(url)?;
                ensure!(
                    matches!(parsed.scheme(), "https" | "http") && parsed.fragment().is_none(),
                    "downloads require HTTP(S) without a fragment"
                );
                if let Download::File { name } = download {
                    native_relative_path(name)?;
                    ensure!(
                        !name.contains('/'),
                        "download filename must be one component"
                    );
                }
                Self::Url {
                    url: parsed.to_string(),
                    download: download.clone(),
                }
            }
            Self::Local { path } => Self::Local {
                path: path.canonicalize()?,
            },
        })
    }
    pub fn id(&self) -> Result<String> {
        let mut canonical = self.canonical()?;
        if let Self::Git { origin, .. } = &mut canonical
            && !origin.starts_with("file:")
        {
            *origin = origin.trim_end_matches(".git").to_owned();
        }
        Ok(sha256(sorted_json(&canonical)?.as_slice())?)
    }
    pub fn provider(&self) -> &'static str {
        match self {
            Self::Git { .. } => "git",
            Self::Url { .. } => "url",
            Self::Local { .. } => "local",
        }
    }
}
impl Recipe {
    pub fn validate(&self) -> Result<()> {
        self.source.canonical()?;
        if let Some(folder) = &self.folder {
            native_relative_path(folder)?;
        }
        if let Some(commit) = &self.commit {
            ensure!(
                matches!(self.source, Source::Git { .. }),
                "commit pins require Git"
            );
            ensure!(
                [40, 64].contains(&commit.len()) && commit.bytes().all(|b| b.is_ascii_hexdigit()),
                "pin must be a full commit ID"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::AppSettings;

    #[test]
    fn defaults_are_app_settings_and_unsupported_recipe_actions_fail() {
        let settings = AppSettings::default();
        assert!(settings.retain_snapshots);
        assert!(!settings.verify_content);
        assert!(!settings.allow_local_fallback);
        assert!(
            serde_json::from_str::<Source>(r#"{"provider":"shell","command":"install"}"#).is_err()
        );
        assert!(serde_json::from_str::<Recipe>(r#"{"source":{"provider":"git","origin":"https://example.com/repo","reference":"main"},"run":"installer"}"#).is_err());
    }

    #[test]
    fn canonical_git_ids_share_folders_but_distinguish_refs() {
        let source = Source::Git {
            origin: "HTTPS://GitHub.com/example/project.git/".into(),
            reference: "main".into(),
        };
        let same = Source::Git {
            origin: "https://github.com/example/project".into(),
            reference: "main".into(),
        };
        assert_eq!(source.id().unwrap(), same.id().unwrap());
        assert_eq!(source.id().unwrap().len(), 64);
        let other = Source::Git {
            origin: "https://github.com/example/project".into(),
            reference: "develop".into(),
        };
        assert_ne!(same.id().unwrap(), other.id().unwrap());
        for folder in ["a", "b"] {
            Recipe {
                source: source.clone(),
                folder: Some(folder.into()),
                commit: None,
            }
            .validate()
            .unwrap();
        }
        let canonical = source.canonical().unwrap();
        assert_eq!(canonical, canonical.canonical().unwrap());
        let ssh = Source::Git {
            origin: "git@example.com:team/repo.git".into(),
            reference: "main".into(),
        };
        let ssh_url = Source::Git {
            origin: "ssh://git@example.com/team/repo".into(),
            reference: "main".into(),
        };
        assert_eq!(ssh.id().unwrap(), ssh_url.id().unwrap());
    }

    #[test]
    fn validates_selection_revision_and_provider_specific_inputs() {
        let source = Source::Url {
            url: "https://example.com/file".into(),
            download: Download::File {
                name: "file".into(),
            },
        };
        let mut recipe = Recipe {
            source,
            folder: None,
            commit: None,
        };
        recipe.validate().unwrap();
        recipe.commit = Some("a".repeat(40));
        assert!(recipe.validate().is_err());
        recipe.commit = None;
        recipe.folder = Some("../escape".into());
        assert!(recipe.validate().is_err());
        assert!(
            Source::Git {
                origin: "https://example.com/repo".into(),
                reference: "--exec=bad".into()
            }
            .canonical()
            .is_err()
        );
        assert!(
            Source::Url {
                url: "file:///secret".into(),
                download: Download::Zip
            }
            .canonical()
            .is_err()
        );
        let dir = tempfile::tempdir().unwrap();
        let local = Source::Local {
            path: dir.path().join("."),
        };
        assert_eq!(
            local.id().unwrap(),
            Source::Local {
                path: dir.path().to_owned()
            }
            .id()
            .unwrap()
        );
    }
}
