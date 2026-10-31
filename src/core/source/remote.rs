use super::{
    AcquireOptions,
    credentials::{self, ProviderToken},
};
use crate::{
    error::{Error, ErrorCode},
    models::{
        source::{Asset, GitPlatform, Resolved, Source},
        store::Selector,
    },
};
use anyhow::{Result, bail, ensure};
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct Release {
    id: u64,
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<ReleaseAsset>,
}
#[derive(Deserialize)]
struct ReleaseAsset {
    id: u64,
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

pub(super) fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .build()
        .into()
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Provider {
    Github,
    Gitlab,
    Gitea,
}
impl Provider {
    fn name(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Gitlab => "gitlab",
            Self::Gitea => "gitea",
        }
    }
}

/// Credentials for one provider operation. Authorization is attached per
/// request and is never forwarded on redirect (ureq's default policy).
pub(crate) struct Auth {
    provider: Provider,
    token: Option<ProviderToken>,
}
impl Auth {
    pub(crate) fn github(options: &AcquireOptions) -> Self {
        Self {
            provider: Provider::Github,
            token: credentials::github(options),
        }
    }
    /// Credentials for a `git_raw` platform, read from its optional token file.
    pub(crate) fn platform(platform: GitPlatform, token_file: Option<&str>) -> Result<Self> {
        Ok(Self {
            provider: match platform {
                GitPlatform::Gitlab => Provider::Gitlab,
                GitPlatform::Gitea => Provider::Gitea,
            },
            token: token_file.map(credentials::token_file).transpose()?,
        })
    }
    pub(crate) fn get(&self, url: &str) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
        self.authorize(agent().get(url))
    }
    pub(crate) fn head(&self, url: &str) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
        self.authorize(agent().head(url))
    }
    fn authorize(
        &self,
        request: ureq::RequestBuilder<ureq::typestate::WithoutBody>,
    ) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
        let request = request.header("User-Agent", "saucepan");
        let Some(token) = &self.token else {
            return request;
        };
        match self.provider {
            Provider::Github => {
                request.header("Authorization", format!("Bearer {}", token.expose()))
            }
            Provider::Gitlab => request.header("PRIVATE-TOKEN", token.expose()),
            Provider::Gitea => request.header("Authorization", format!("token {}", token.expose())),
        }
    }
    /// Send a request, reporting provider authorization failures clearly.
    pub(crate) fn call(
        &self,
        request: ureq::RequestBuilder<ureq::typestate::WithoutBody>,
    ) -> Result<ureq::http::Response<ureq::Body>> {
        match request.call() {
            Err(ureq::Error::StatusCode(status)) => Err(self.denied(status).map_or_else(
                || anyhow::Error::from(ureq::Error::StatusCode(status)),
                anyhow::Error::from,
            )),
            other => Ok(other?),
        }
    }
    fn denied(&self, status: u16) -> Option<Error> {
        let sent = self.token.is_some();
        let message = match (self.provider, status, sent) {
            (Provider::Github, 401 | 403, _) => format!(
                "GitHub denied access (HTTP {status}); run `gh auth login`, or the API rate limit was reached"
            ),
            (Provider::Github, 404, false) => {
                "GitHub returned HTTP 404 without a token; if the repository is private, run `gh auth login`".into()
            }
            (provider, 401 | 403, _) if provider != Provider::Github => format!(
                "{} denied access (HTTP {status}); check the configured token_file",
                provider.name()
            ),
            _ => return None,
        };
        let mut error = Error::new(ErrorCode::SourceError, message);
        error
            .details
            .insert("provider".into(), self.provider.name().into());
        error
            .details
            .insert("http_status".into(), status.to_string());
        error.details.insert("token_sent".into(), sent.to_string());
        Some(error)
    }
}

pub(crate) fn github_api(options: &AcquireOptions) -> Result<String> {
    loopback_override(options.test_github_api.as_deref(), "https://api.github.com")
}
pub(crate) fn github_raw(options: &AcquireOptions) -> Result<String> {
    loopback_override(
        options.test_github_raw.as_deref(),
        "https://raw.githubusercontent.com",
    )
}
fn git_raw_base(host: &str, options: &AcquireOptions) -> Result<String> {
    loopback_override(options.test_git_raw.as_deref(), &format!("https://{host}"))
}
fn loopback_override(test: Option<&str>, default: &str) -> Result<String> {
    let base = test.unwrap_or(default).trim_end_matches('/');
    if test.is_some() {
        ensure!(
            base.starts_with("http://127.0.0.1:") || base.starts_with("http://localhost:"),
            "test endpoint must be loopback"
        );
    }
    Ok(base.to_owned())
}

pub(super) fn repo_path(repo: &str) -> Result<()> {
    let parts: Vec<_> = repo.split('/').collect();
    ensure!(
        parts.len() == 2
            && parts.iter().all(|p| !p.is_empty()
                && *p != "."
                && *p != ".."
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))),
        "expected owner/repo"
    );
    Ok(())
}
fn encode_path(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
/// Percent-encode each `/`-separated segment, keeping the separators.
fn encode_segments(value: &str) -> String {
    value
        .split('/')
        .map(encode_path)
        .collect::<Vec<_>>()
        .join("/")
}

/// A Git selector's reference and whether it already names an exact commit.
pub(crate) fn reference(selector: &Selector) -> Result<(&str, bool)> {
    let (reference, pinned) = match selector {
        Selector::Ref { reference } => (reference.as_str(), false),
        Selector::Tag { tag } => (tag.as_str(), false),
        Selector::Commit { commit } => (commit.as_str(), true),
        _ => bail!("Git source requires ref, tag or commit"),
    };
    ensure!(!reference.is_empty(), "empty Git reference");
    Ok((reference, pinned))
}

fn exact_commit(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Resolve a GitHub ref, tag or commit to the commit ID it names now.
pub(crate) fn github_commit(
    repo: &str,
    selector: &Selector,
    options: &AcquireOptions,
) -> Result<String> {
    repo_path(repo)?;
    let (reference, pinned) = reference(selector)?;
    if pinned {
        ensure!(
            exact_commit(reference),
            Error::new(
                ErrorCode::InvalidRequest,
                "commit selector requires a full commit ID"
            )
        );
        return Ok(reference.to_ascii_lowercase());
    }
    let base = github_api(options)?;
    let auth = Auth::github(options);
    let commit = auth
        .call(
            auth.get(&format!(
                "{base}/repos/{repo}/commits/{}",
                encode_segments(reference)
            ))
            .header("Accept", "application/vnd.github.sha"),
        )?
        .body_mut()
        .with_config()
        .limit(1024)
        .read_to_string()?
        .trim()
        .to_ascii_lowercase();
    ensure!(
        exact_commit(&commit),
        "GitHub returned an invalid commit ID"
    );
    Ok(commit)
}

/// The raw-file URL of a `github_raw` file at an exact commit.
pub(crate) fn github_raw_url(
    repo: &str,
    path: &str,
    commit: &str,
    options: &AcquireOptions,
) -> Result<String> {
    Ok(format!(
        "{}/{repo}/{commit}/{}",
        github_raw(options)?,
        encode_segments(path)
    ))
}

/// The platform raw-file route of a `git_raw` source for a selector.
pub(crate) fn git_raw_url(
    source: &Source,
    selector: &Selector,
    options: &AcquireOptions,
) -> Result<String> {
    let Source::GitRaw {
        host,
        platform,
        repo,
        path,
        ..
    } = source
    else {
        bail!("not a git_raw source")
    };
    let (reference, _) = reference(selector)?;
    let route = match platform {
        GitPlatform::Gitlab => "-/raw",
        GitPlatform::Gitea => "raw",
    };
    Ok(format!(
        "{}/{}/{route}/{}/{}",
        git_raw_base(host, options)?,
        encode_segments(repo),
        encode_segments(reference),
        encode_segments(path)
    ))
}

fn header(response: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

pub(crate) fn discover(
    source: &Source,
    selector: &Selector,
    options: &AcquireOptions,
) -> Result<Resolved> {
    let agent = agent();
    match source {
        Source::GithubRelease { repo } => {
            repo_path(repo)?;
            let base = github_api(options)?;
            let suffix = match selector {
                Selector::LatestStable => "latest".into(),
                Selector::Tag { tag } => {
                    ensure!(!tag.is_empty(), "empty tag");
                    format!("tags/{}", encode_path(tag))
                }
                _ => bail!("release source requires latest_stable or tag"),
            };
            let auth = Auth::github(options);
            let mut response = auth.call(
                auth.get(&format!("{base}/repos/{repo}/releases/{suffix}"))
                    .header("Accept", "application/vnd.github+json"),
            )?;
            let release: Release = response.body_mut().read_json()?;
            ensure!(!release.draft, "draft release is unavailable");
            if let Selector::Tag { tag } = selector {
                ensure!(release.tag_name == *tag, "release tag mismatch");
            }
            ensure!(
                !matches!(selector, Selector::LatestStable) || !release.prerelease,
                "latest stable resolved a prerelease"
            );
            Ok(Resolved::Release {
                id: release.id,
                tag: release.tag_name,
                assets: release
                    .assets
                    .into_iter()
                    .map(|a| Asset {
                        id: a.id,
                        name: a.name,
                        url: a.browser_download_url,
                        digest: a.digest,
                    })
                    .collect(),
            })
        }
        Source::Url { url } => {
            ensure!(
                matches!(selector, Selector::Url),
                "URL source requires url selector"
            );
            let response = agent.head(url).call()?;
            Ok(Resolved::Url {
                url: url.clone(),
                etag: response
                    .headers()
                    .get("etag")
                    .and_then(|h| h.to_str().ok())
                    .map(str::to_owned),
                last_modified: response
                    .headers()
                    .get("last-modified")
                    .and_then(|h| h.to_str().ok())
                    .map(str::to_owned),
            })
        }
        Source::GithubRaw { repo, .. } | Source::GithubSource { repo } => Ok(Resolved::Git {
            commit: github_commit(repo, selector, options)?,
        }),
        Source::GitRaw {
            platform,
            token_file,
            ..
        } => {
            let url = git_raw_url(source, selector, options)?;
            let auth = Auth::platform(*platform, token_file.as_deref())?;
            let response = auth.call(auth.head(&url))?;
            Ok(Resolved::Url {
                etag: header(&response, "etag"),
                last_modified: header(&response, "last-modified"),
                url,
            })
        }
        _ => bail!("not a remote download source"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// Serve one request with `status` and return its head.
    fn capture(status: u16, send: impl FnOnce(&str)) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut data = Vec::new();
            let mut buf = [0; 1024];
            while !data.windows(4).any(|b| b == b"\r\n\r\n") {
                let n = stream.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..n]);
            }
            let reply =
                format!("HTTP/1.1 {status} x\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            stream.write_all(reply.as_bytes()).unwrap();
            String::from_utf8(data).unwrap().to_lowercase()
        });
        send(&base);
        server.join().unwrap()
    }

    fn platform(platform: GitPlatform, token: Option<&str>) -> (Auth, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let file = token.map(|token| {
            let path = dir.path().join("token");
            std::fs::write(&path, token).unwrap();
            path.to_str().unwrap().to_owned()
        });
        (Auth::platform(platform, file.as_deref()).unwrap(), dir)
    }

    #[test]
    fn platform_tokens_use_platform_headers() {
        let (gitlab, _a) = platform(GitPlatform::Gitlab, Some("glpat-1"));
        let head = capture(200, |base| {
            gitlab.call(gitlab.get(&format!("{base}/x"))).unwrap();
        });
        assert!(head.contains("private-token: glpat-1"));
        let (gitea, _b) = platform(GitPlatform::Gitea, Some("tea-1"));
        let head = capture(200, |base| {
            gitea.call(gitea.get(&format!("{base}/x"))).unwrap();
        });
        assert!(head.contains("authorization: token tea-1"));
        let (anonymous, _c) = platform(GitPlatform::Gitea, None);
        let head = capture(200, |base| {
            anonymous.call(anonymous.get(&format!("{base}/x"))).unwrap();
        });
        assert!(!head.contains("authorization") && !head.contains("private-token"));
    }

    #[test]
    fn platform_denial_points_at_token_file() {
        let (gitlab, _dir) = platform(GitPlatform::Gitlab, Some("glpat-secret"));
        let mut failure = None;
        capture(403, |base| {
            failure = Some(gitlab.call(gitlab.get(&format!("{base}/x"))).unwrap_err());
        });
        let failure = failure.unwrap();
        let error = Error::from_anyhow(&failure);
        assert_eq!(error.code, ErrorCode::SourceError);
        assert_eq!(error.details["provider"], "gitlab");
        assert_eq!(error.details["http_status"], "403");
        assert_eq!(error.details["token_sent"], "true");
        assert!(error.message.contains("token_file"));
        assert!(!format!("{failure:?}").contains("glpat-secret"));
    }
}
