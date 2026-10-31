//! Provider credentials: borrowed from `gh` or read from a configured file.
//! Tokens live in memory for one operation and are never serialized.
use super::AcquireOptions;
use crate::{
    error::{Error, ErrorCode},
    models::source::Source,
    utils::{process, source::git::HttpAuth},
};
use anyhow::Result;
use std::{path::PathBuf, process::Command, time::Duration};

const GH_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub(crate) struct ProviderToken(String);
impl std::fmt::Debug for ProviderToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProviderToken(<redacted>)")
    }
}
impl ProviderToken {
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

/// Token for github.com from `gh`, or none when `gh` cannot supply one.
pub(crate) fn github(options: &AcquireOptions) -> Option<ProviderToken> {
    github_within(options, GH_TIMEOUT)
}

fn github_within(options: &AcquireOptions, timeout: Duration) -> Option<ProviderToken> {
    // Fixture endpoints must never receive the developer's real login.
    let executable = match &options.test_gh {
        Some(path) => path.clone(),
        None if options.uses_test_endpoints() => return None,
        None => PathBuf::from("gh"),
    };
    let mut command = Command::new(executable);
    command.args(["auth", "token", "--hostname", "github.com"]);
    let output = process::run(command, timeout, 4096).ok()?;
    if !output.status.success() || output.truncated {
        return None;
    }
    let token = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!token.is_empty()).then_some(ProviderToken(token))
}

/// Clone credentials: `github_repo` borrows from `gh`, `git_repo` only reads
/// its configured token file.
pub(crate) fn repository(
    source: &Source,
    origin: &str,
    options: &AcquireOptions,
) -> Result<Option<HttpAuth>> {
    match source {
        Source::GithubRepo { .. } => github(options)
            .map(|token| HttpAuth::basic(origin, "x-access-token", token.expose()))
            .transpose(),
        Source::GitRepo {
            token_file: Some(file),
            ..
        } => Ok(Some(HttpAuth::basic(
            origin,
            "oauth2",
            token_file(file)?.expose(),
        )?)),
        _ => Ok(None),
    }
}

/// Report a Git host's authorization refusal like other provider denials.
pub(crate) fn git_denied(error: anyhow::Error, source: &Source, token_sent: bool) -> anyhow::Error {
    let text = error.to_string();
    let status = if text.contains("returned error: 403") {
        "403"
    } else if text.contains("Authentication failed")
        || text.contains("returned error: 401")
        || text.contains("could not read Username")
    {
        "401"
    } else {
        return error;
    };
    let (provider, hint) = match source {
        Source::GithubRepo { .. } => ("github", "run `gh auth login`"),
        _ => ("git", "check the configured token_file"),
    };
    let mut denied = Error::new(
        ErrorCode::SourceError,
        format!("Git host denied access (HTTP {status}); {hint}"),
    );
    denied.details.insert("provider".into(), provider.into());
    denied.details.insert("http_status".into(), status.into());
    denied
        .details
        .insert("token_sent".into(), token_sent.to_string());
    denied.into()
}

/// Read a configured token file; `~` expands to the current user's home.
pub(crate) fn token_file(path: &str) -> Result<ProviderToken> {
    let resolved = expand_home(path)?;
    let failure = |reason: &str| {
        let mut error = Error::new(
            ErrorCode::SourceError,
            format!("token file {path}: {reason}"),
        );
        error.details.insert("token_file".into(), path.into());
        error
    };
    let bytes = std::fs::read(&resolved).map_err(|e| failure(&e.to_string()))?;
    let token = String::from_utf8(bytes)
        .map_err(|_| failure("not UTF-8"))?
        .trim()
        .to_owned();
    if token.is_empty() {
        return Err(failure("empty").into());
    }
    Ok(ProviderToken(token))
}

fn expand_home(path: &str) -> Result<PathBuf> {
    let rest = match path.strip_prefix('~') {
        None => return Ok(PathBuf::from(path)),
        Some("") => "",
        Some(rest) if rest.starts_with(['/', '\\']) => &rest[1..],
        Some(_) => return Ok(PathBuf::from(path)),
    };
    let home = std::env::home_dir().ok_or_else(|| {
        Error::new(
            ErrorCode::SourceError,
            "cannot expand ~ in token file: no home directory",
        )
    })?;
    Ok(home.join(rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_gh(dir: &std::path::Path, body: &str) -> PathBuf {
        #[cfg(windows)]
        {
            let path = dir.join("gh.cmd");
            std::fs::write(&path, format!("@echo off\r\n{body}\r\n")).unwrap();
            path
        }
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = dir.join("gh");
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path
        }
    }

    fn options(gh: PathBuf) -> AcquireOptions {
        AcquireOptions {
            test_gh: Some(gh),
            ..Default::default()
        }
    }

    #[test]
    fn gh_token_is_borrowed_and_redacted() {
        let dir = tempfile::tempdir().unwrap();
        let token = github(&options(fake_gh(dir.path(), "echo gho_secret"))).unwrap();
        assert_eq!(token.expose(), "gho_secret");
        assert!(!format!("{token:?}").contains("gho_secret"));
    }

    #[test]
    fn unusable_gh_means_anonymous() {
        let dir = tempfile::tempdir().unwrap();
        assert!(github(&options(dir.path().join("missing-gh"))).is_none());
        #[cfg(windows)]
        let failing = "exit /b 1";
        #[cfg(not(windows))]
        let failing = "exit 1";
        assert!(github(&options(fake_gh(dir.path(), failing))).is_none());
        let empty = tempfile::tempdir().unwrap();
        assert!(github(&options(fake_gh(empty.path(), "echo."))).is_none());
    }

    #[test]
    fn hanging_gh_is_terminated() {
        let dir = tempfile::tempdir().unwrap();
        #[cfg(windows)]
        let hang = "ping -n 30 127.0.0.1 >nul";
        #[cfg(not(windows))]
        let hang = "sleep 30";
        let started = std::time::Instant::now();
        let gh = options(fake_gh(dir.path(), hang));
        assert!(github_within(&gh, Duration::from_secs(1)).is_none());
        assert!(started.elapsed() < Duration::from_secs(20));
    }

    #[test]
    fn fixture_endpoints_never_use_real_gh() {
        let options = AcquireOptions {
            test_github_api: Some("http://127.0.0.1:1".into()),
            ..Default::default()
        };
        assert!(github(&options).is_none());
    }

    #[test]
    fn token_file_is_trimmed_and_failures_name_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("token");
        std::fs::write(&file, "glpat-x\n").unwrap();
        assert_eq!(
            token_file(file.to_str().unwrap()).unwrap().expose(),
            "glpat-x"
        );
        std::fs::write(&file, "  \n").unwrap();
        let empty = token_file(file.to_str().unwrap()).unwrap_err();
        assert!(empty.to_string().contains("empty"));
        let missing = dir.path().join("absent");
        let error = Error::from_anyhow(&token_file(missing.to_str().unwrap()).unwrap_err());
        assert_eq!(error.code, ErrorCode::SourceError);
        assert!(error.message.contains(missing.to_str().unwrap()));
    }

    #[test]
    fn home_relative_token_file_resolves_beneath_home() {
        let home = std::env::home_dir().unwrap();
        assert_eq!(
            expand_home("~/.config/saucepan/gitlab.token").unwrap(),
            home.join(".config/saucepan/gitlab.token")
        );
        assert_eq!(expand_home("/abs/t").unwrap(), PathBuf::from("/abs/t"));
    }

    #[test]
    fn github_repo_borrows_gh_and_git_repo_never_does() {
        let dir = tempfile::tempdir().unwrap();
        let gh = options(fake_gh(dir.path(), "echo gho_secret"));
        let github = Source::GithubRepo { repo: "o/r".into() };
        let auth = repository(&github, "https://github.com/o/r.git", &gh).unwrap();
        assert!(auth.is_some_and(|auth| !format!("{auth:?}").contains("gho_secret")));
        let plain = Source::GitRepo {
            origin: "https://github.com/o/r.git".into(),
            token_file: None,
        };
        assert!(
            repository(&plain, "https://github.com/o/r.git", &gh)
                .unwrap()
                .is_none()
        );
    }
}
