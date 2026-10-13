use super::{AcquireOptions, Prepared};
use crate::{
    core::{Store, policy::select_asset},
    models::{
        index::Record,
        source::{Asset, Discovery, Resolved, Source},
        store::{Availability, Format, Request, Selector},
    },
    utils::{
        archive::extract_zip,
        lock::{LockGuard, LockMode},
        path::relative_path,
        transfer::copy_bounded,
        tree::{self, TreeLimits},
    },
};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, time::Duration};

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
impl Store {
    pub(super) fn prepare_remote(
        &self,
        request: &Request,
        options: &AcquireOptions,
        temp: tempfile::TempDir,
    ) -> Result<Prepared> {
        self.prepare_remote_resolved(request, options, temp, None)
    }

    pub(super) fn prepare_remote_resolved(
        &self,
        request: &Request,
        options: &AcquireOptions,
        temp: tempfile::TempDir,
        exact: Option<&Resolved>,
    ) -> Result<Prepared> {
        ensure!(
            matches!(request.selection.format, Format::Raw | Format::Zip),
            "downloads require raw or zip format"
        );
        let _source_lock = LockGuard::acquire(
            &self
                .root
                .join(format!(".locks/source-{}", request.source_uuid)),
            LockMode::Exclusive,
            Duration::from_secs(90),
        )?;
        let mut record = self.source(request.source_uuid)?;
        let key = serde_json::to_string(&request.selector)?;
        let now = options.now()?;
        let previous = record.discovery.get(&key).cloned();
        let fresh = previous.as_ref().is_some_and(|d| {
            now.checked_sub(d.checked_at)
                .is_some_and(|age| age < record.refresh.ttl_seconds)
        });
        let mut update_checked = false;
        let mut stale = false;
        let discovered = if let Some(resolved) = exact {
            resolved.clone()
        } else if fresh && !options.force_refresh {
            previous.as_ref().unwrap().resolved.clone()
        } else {
            match discover(&record.source, &request.selector, options) {
                Ok(resolved) => {
                    update_checked = true;
                    record.discovery.insert(
                        key,
                        Discovery {
                            checked_at: now,
                            resolved: resolved.clone(),
                        },
                    );
                    let _guard = self.lock()?;
                    self.save_source(&record)?;
                    resolved
                }
                Err(error) => {
                    if options.allow_stale {
                        stale = true;
                        previous
                            .as_ref()
                            .context("discovery failed without a cached result")?
                            .resolved
                            .clone()
                    } else {
                        return Err(error.context("source discovery failed"));
                    }
                }
            }
        };
        let (resolved, url, filename, expected) = match discovered {
            Resolved::Release { id, tag, assets } => {
                let asset = select_asset(&assets, &request.selection)?;
                let resolved = Resolved::Release {
                    id,
                    tag,
                    assets: vec![asset.clone()],
                };
                (
                    resolved,
                    asset.url.clone(),
                    asset.name.clone(),
                    asset.digest.clone(),
                )
            }
            Resolved::Url { ref url, .. } => (
                discovered.clone(),
                url.clone(),
                request
                    .selection
                    .filename
                    .clone()
                    .context("URL download requires filename")?,
                None,
            ),
            _ => bail!("invalid discovery cache type"),
        };
        // A cache hit does not require re-downloading bytes already present and verified.
        let can_reuse = match &resolved {
            Resolved::Url {
                etag,
                last_modified,
                ..
            } => !update_checked || etag.is_some() || last_modified.is_some(),
            _ => true,
        };
        if can_reuse && exact.is_none() {
            let _guard = self.lock()?;
            for entry in self.records()?.into_iter().filter_map(|r| {
                if let Record::Output(e) = r {
                    Some(e)
                } else {
                    None
                }
            }) {
                if entry.state == Availability::Ready
                    && entry.request.source_uuid == request.source_uuid
                    && entry.request.selector == request.selector
                    && entry.request.selection == request.selection
                    && entry.resolved == resolved
                {
                    let source = match self.verified_path(&entry) {
                        Ok(source) => source,
                        Err(error)
                            if crate::error::Error::from_anyhow(&error).code
                                == crate::error::ErrorCode::ContentUnavailable =>
                        {
                            continue;
                        }
                        Err(error) => return Err(error),
                    };
                    tree::copy_tree(&source, &temp.path().join("content"), TreeLimits::default())?;
                    return Ok(Prepared {
                        temp,
                        resolved,
                        dependencies: entry.dependencies,
                        update_checked,
                        stale,
                    });
                }
            }
        }
        let agent = agent();
        let mut response = agent.get(&url).header("User-Agent", "saucepan").call()?;
        let asset = temp.path().join("download");
        let mut output = fs::File::create(&asset)?;
        let transferred = copy_bounded(
            response.body_mut().as_reader(),
            &mut output,
            TreeLimits::default().bytes,
            true,
        )?;
        drop(output);
        let digest = transferred.sha256.context("download digest missing")?;
        if let Some(expected) = expected {
            let expected = expected
                .strip_prefix("sha256:")
                .context("unsupported release digest algorithm")?;
            ensure!(
                digest.eq_ignore_ascii_case(expected),
                crate::error::Error::new(
                    crate::error::ErrorCode::IntegrityError,
                    "release checksum mismatch"
                )
            );
        }
        if let Some(expected) = &request.selection.sha256 {
            ensure!(
                digest.eq_ignore_ascii_case(expected),
                crate::error::Error::new(
                    crate::error::ErrorCode::IntegrityError,
                    "download checksum mismatch"
                )
            );
        }
        let content = temp.path().join("content");
        match request.selection.format {
            Format::Zip => extract_zip(
                fs::File::open(asset)?,
                &content,
                request.selection.folder.as_deref(),
                TreeLimits::default(),
            )?,
            Format::Raw => {
                ensure!(
                    request.selection.folder.is_none(),
                    "raw download has no folder selection"
                );
                let name = request.selection.filename.as_deref().unwrap_or(&filename);
                relative_path(name)?;
                ensure!(!name.contains('/'), "raw filename must be one component");
                fs::create_dir(&content)?;
                fs::rename(asset, content.join(name))?;
                #[cfg(unix)]
                if request.selection.bin.iter().any(|p| p == name) {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(content.join(name), fs::Permissions::from_mode(0o755))?;
                }
            }
            Format::Natural => unreachable!(),
        }
        Ok(Prepared {
            temp,
            resolved,
            dependencies: BTreeMap::new(),
            update_checked,
            stale,
        })
    }
}
