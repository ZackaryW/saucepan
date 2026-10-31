use crate::{
    error::{Error, ErrorCode},
    models::source::Source,
    utils::{
        path::{plain_metadata, relative_path},
        source::git,
    },
};
use anyhow::{Result, ensure};
use std::fs;

/// Parse a source definition, naming the replacements for the removed `git` type.
pub(crate) fn parse(input: &[u8]) -> Result<Source> {
    serde_json::from_slice(input).map_err(|error| {
        let removed = serde_json::from_slice::<serde_json::Value>(input)
            .is_ok_and(|value| value.get("type").and_then(|t| t.as_str()) == Some("git"));
        if removed {
            Error::new(
                ErrorCode::InvalidRequest,
                "source type `git` was removed; use `git_repo` or `github_repo`",
            )
            .into()
        } else {
            error.into()
        }
    })
}

/// Canonicalize within one source type; types are never rewritten into others.
pub(crate) fn normalize(source: Source) -> Result<Source> {
    Ok(match source {
        Source::GithubRepo { repo } => Source::GithubRepo {
            repo: github_repo(&repo)?,
        },
        Source::GithubRaw { repo, path } => {
            relative_path(&path)?;
            Source::GithubRaw {
                repo: github_repo(&repo)?,
                path,
            }
        }
        Source::GithubSource { repo } => Source::GithubSource {
            repo: github_repo(&repo)?,
        },
        Source::GitRepo { origin, token_file } => {
            let mut origin = git::normalize(&origin)?;
            if let Some(repo) = origin.strip_prefix("https://github.com/") {
                origin = format!("https://github.com/{}.git", github_repo(repo)?);
            }
            if let Some(file) = &token_file {
                ensure!(!file.is_empty(), "empty token file path");
                ensure!(
                    origin.starts_with("https://") || origin.starts_with("http://"),
                    "token_file requires an HTTP(S) origin"
                );
            }
            Source::GitRepo { origin, token_file }
        }
        Source::GitRaw {
            host,
            platform,
            repo,
            path,
            token_file,
        } => {
            relative_path(&path)?;
            ensure!(
                token_file.as_ref().is_none_or(|file| !file.is_empty()),
                "empty token file path"
            );
            Source::GitRaw {
                host: raw_host(&host)?,
                platform,
                repo: namespaced_repo(&repo)?,
                path,
                token_file,
            }
        }
        Source::Local { path } => {
            plain_metadata(&path)?;
            Source::Local {
                path: fs::canonicalize(path)?,
            }
        }
        other => other,
    })
}

/// Clone origin of a repository-backed source, after normalization.
pub(crate) fn repository_origin(source: &Source) -> Result<Option<String>> {
    Ok(match normalize(source.clone())? {
        Source::GitRepo { origin, .. } => Some(origin),
        Source::GithubRepo { repo } => Some(format!("https://github.com/{repo}.git")),
        _ => None,
    })
}

pub(crate) fn is_repository(source: &Source) -> bool {
    matches!(source, Source::GitRepo { .. } | Source::GithubRepo { .. })
}

fn github_repo(repo: &str) -> Result<String> {
    let repo = repo.trim_end_matches(".git");
    super::remote::repo_path(repo)?;
    Ok(repo.to_lowercase())
}

// GitLab namespaces nest, so two or more segments are accepted.
fn namespaced_repo(repo: &str) -> Result<String> {
    let parts: Vec<_> = repo.split('/').collect();
    ensure!(
        parts.len() >= 2
            && parts.iter().all(|p| !p.is_empty()
                && *p != "."
                && *p != ".."
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))),
        "expected namespace/repo"
    );
    Ok(repo.to_owned())
}

fn raw_host(host: &str) -> Result<String> {
    ensure!(
        !host.is_empty()
            && !host.contains(['/', '@', '?', '#', '\\', ' '])
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-:[]".contains(&b)),
        "host must be a bare host name"
    );
    let url = url::Url::parse(&format!("https://{host}/"))?;
    ensure!(
        url.host_str().is_some() && url.username().is_empty() && url.password().is_none(),
        "host must be a bare host name"
    );
    Ok(host.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::source::GitPlatform;

    fn raw(host: &str, repo: &str, path: &str) -> Result<Source> {
        normalize(Source::GitRaw {
            host: host.into(),
            platform: GitPlatform::Gitlab,
            repo: repo.into(),
            path: path.into(),
            token_file: None,
        })
    }

    #[test]
    fn github_repo_stays_its_own_type() {
        assert_eq!(
            normalize(Source::GithubRepo {
                repo: "Owner/Repo.git".into()
            })
            .unwrap(),
            Source::GithubRepo {
                repo: "owner/repo".into()
            }
        );
    }

    #[test]
    fn raw_inputs_are_validated_before_network() {
        assert!(raw("gitlab.example.com", "group/sub/repo", "a/b.md").is_ok());
        assert!(raw("gitlab.example.com", "group/repo", "../secrets").is_err());
        assert!(raw("user:pass@gitlab.example.com", "group/repo", "a").is_err());
        assert!(raw("gitlab.example.com/path", "group/repo", "a").is_err());
        assert!(raw("gitlab.example.com", "repo", "a").is_err());
        assert!(
            normalize(Source::GithubRaw {
                repo: "owner/repo".into(),
                path: "a/./b".into()
            })
            .is_err()
        );
    }

    #[test]
    fn token_file_requires_http_origin() {
        let source = |origin: &str| Source::GitRepo {
            origin: origin.into(),
            token_file: Some("~/token".into()),
        };
        assert!(normalize(source("ssh://git@example.com/a/b")).is_err());
        assert!(normalize(source("https://example.com/a/b")).is_ok());
    }

    #[test]
    fn removed_git_type_does_not_deserialize() {
        let parsed: Result<Source, _> =
            serde_json::from_str(r#"{"type":"git","origin":"https://example.com/a"}"#);
        let message = parsed.unwrap_err().to_string();
        assert!(message.contains("git_repo") && message.contains("github_repo"));
        let error = Error::from_anyhow(
            &parse(br#"{"type":"git","origin":"https://example.com/a"}"#).unwrap_err(),
        );
        assert_eq!(error.code, ErrorCode::InvalidRequest);
        assert!(error.message.contains("git_repo") && error.message.contains("github_repo"));
    }

    #[test]
    fn every_variant_round_trips() {
        for source in [
            Source::GithubRepo { repo: "o/r".into() },
            Source::GithubRaw {
                repo: "o/r".into(),
                path: "a.md".into(),
            },
            Source::GithubSource { repo: "o/r".into() },
            Source::GitRepo {
                origin: "https://example.com/a".into(),
                token_file: Some("~/t".into()),
            },
            Source::GitRaw {
                host: "h".into(),
                platform: GitPlatform::Gitea,
                repo: "o/r".into(),
                path: "a".into(),
                token_file: None,
            },
        ] {
            let text = serde_json::to_string(&source).unwrap();
            assert_eq!(serde_json::from_str::<Source>(&text).unwrap(), source);
        }
    }
}
