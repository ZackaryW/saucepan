use super::AcquireOptions;
use crate::models::{
    source::{Asset, Resolved, Source},
    store::Selector,
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
pub(crate) fn discover(
    source: &Source,
    selector: &Selector,
    options: &AcquireOptions,
) -> Result<Resolved> {
    let agent = agent();
    match source {
        Source::GithubRelease { repo } => {
            repo_path(repo)?;
            let base = options
                .test_github_api
                .as_deref()
                .unwrap_or("https://api.github.com")
                .trim_end_matches('/');
            if options.test_github_api.is_some() {
                ensure!(
                    base.starts_with("http://127.0.0.1:") || base.starts_with("http://localhost:"),
                    "test endpoint must be loopback"
                );
            }
            let suffix = match selector {
                Selector::LatestStable => "latest".into(),
                Selector::Tag { tag } => {
                    ensure!(!tag.is_empty(), "empty tag");
                    format!("tags/{}", encode_path(tag))
                }
                _ => bail!("release source requires latest_stable or tag"),
            };
            let mut response = agent
                .get(format!("{base}/repos/{repo}/releases/{suffix}"))
                .header("User-Agent", "saucepan")
                .header("Accept", "application/vnd.github+json")
                .call()?;
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
        _ => bail!("not a remote download source"),
    }
}
