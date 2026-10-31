//! Provider retrieval into caller-supplied private working data. No Store access.
use super::{AcquireOptions, credentials, git, remote};
use crate::{
    core::policy::select_asset,
    error::{Error, ErrorCode},
    models::{
        policy::PayloadVerification,
        retrieval::{MaterialKind, RetrievalPlan, VerificationEvidence},
        source::{Resolved, Source},
        store::{Selection, Selector},
    },
    utils::{
        path::plain_metadata,
        source::git::{Git, RepositoryMaterial},
        transfer::copy_bounded,
        tree::{TreeLimits, copy_tree_unhashed},
    },
};
use anyhow::{Result, bail, ensure};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub(crate) struct Material {
    pub path: PathBuf,
    pub kind: MaterialKind,
    pub resolved: Resolved,
    pub dependencies: BTreeMap<String, String>,
    pub verification: VerificationEvidence,
}

pub(crate) fn validate(source: &Source, request: &RetrievalPlan) -> Result<()> {
    validate_selector(source, request)?;
    if matches!(source, Source::Local { path } if plain_metadata(path)?.is_dir()) {
        ensure!(
            request.expected_sha256.is_none()
                && request.verification != PayloadVerification::Required,
            Error::new(
                ErrorCode::InvalidRequest,
                "expected payload digest requires file material"
            )
        );
    }
    Ok(())
}

/// Structural provider preflight without touching upstream files or the network.
pub(crate) fn validate_selector(source: &Source, request: &RetrievalPlan) -> Result<()> {
    let valid = matches!(
        (source, &request.selector),
        (Source::Local { .. }, Selector::Local)
            | (Source::Url { .. }, Selector::Url)
            | (
                Source::GithubRelease { .. },
                Selector::LatestStable | Selector::Tag { .. }
            )
            | (
                Source::GitRepo { .. }
                    | Source::GithubRepo { .. }
                    | Source::GithubRaw { .. }
                    | Source::GithubSource { .. }
                    | Source::GitRaw { .. },
                Selector::Ref { .. } | Selector::Tag { .. } | Selector::Commit { .. }
            )
    );
    ensure!(
        valid,
        Error::new(
            ErrorCode::InvalidRequest,
            "source and selector are incompatible"
        )
    );
    if let Source::Url { url } = source {
        let url = url::Url::parse(url)?;
        ensure!(
            matches!(url.scheme(), "http" | "https"),
            "unsupported download transport"
        );
    }
    if git::is_repository(source) {
        ensure!(
            request.expected_sha256.is_none()
                && request.verification != PayloadVerification::Required,
            Error::new(
                ErrorCode::InvalidRequest,
                "expected payload digest requires file material"
            )
        );
    }
    Ok(())
}

pub(crate) fn obtain(
    source: &Source,
    request: &RetrievalPlan,
    work: &Path,
    scratch: &Path,
    options: &AcquireOptions,
    exact: Option<&Resolved>,
) -> Result<Material> {
    validate(source, request)?;
    match git::normalize(source.clone())? {
        source if git::is_repository(&source) => {
            let origin = git::repository_origin(&source)?.expect("repository source");
            let (reference, pinned) = match &request.selector {
                Selector::Ref { reference } => (reference.as_str(), false),
                Selector::Tag { tag } => (tag.as_str(), false),
                Selector::Commit { commit } => (commit.as_str(), true),
                _ => unreachable!("validated Git selector"),
            };
            let path = work.join("repo");
            let git = Git {
                auth: credentials::repository(&source, &origin, options)?,
                ..Git::default()
            };
            let denied =
                |error: anyhow::Error| credentials::git_denied(error, &source, git.auth.is_some());
            git.repository(&origin, &path).map_err(denied)?;
            let commit = match exact {
                Some(Resolved::Git { commit }) => git.resolve(&path, commit, true),
                Some(_) => bail!("invalid Git provenance"),
                None => git.resolve(&path, reference, pinned),
            }
            .map_err(denied)?;
            let dependencies = git
                .hydrate(
                    RepositoryMaterial {
                        origin: &origin,
                        repository: &path,
                        commit: &commit,
                        dependencies: work,
                    },
                    scratch,
                )?
                .dependencies;
            Ok(Material {
                path,
                kind: MaterialKind::GitRepository { bare: true },
                resolved: Resolved::Git { commit },
                dependencies,
                verification: VerificationEvidence {
                    git_objects_validated: true,
                    ..Default::default()
                },
            })
        }
        Source::Local { path } => {
            let metadata = plain_metadata(&path)?;
            let destination = work.join("material");
            let (kind, verification) = if metadata.is_dir() {
                copy_tree_unhashed(&path, &destination, TreeLimits::default())?;
                (MaterialKind::Directory, VerificationEvidence::default())
            } else {
                let expected = expected_digest(request, None)?;
                let mut output = fs::File::create(&destination)?;
                let transferred = copy_bounded(
                    fs::File::open(&path)?,
                    &mut output,
                    TreeLimits::default().bytes,
                    expected.is_some(),
                )?;
                let after = plain_metadata(&path)?;
                ensure!(
                    metadata.len() == transferred.bytes
                        && after.len() == metadata.len()
                        && after.modified()? == metadata.modified()?,
                    "local input changed during capture"
                );
                fs::set_permissions(&destination, metadata.permissions())?;
                (
                    MaterialKind::File,
                    verify_digest(expected.as_deref(), transferred.sha256.as_deref())?,
                )
            };
            Ok(Material {
                path: destination,
                kind,
                resolved: Resolved::Local,
                dependencies: BTreeMap::new(),
                verification,
            })
        }
        source @ (Source::Url { .. } | Source::GithubRelease { .. }) => {
            let (resolved, url, expected) = match &source {
                Source::Url { url } => (None, url.clone(), expected_digest(request, None)?),
                _ => {
                    let Resolved::Release { id, tag, assets } = (match exact {
                        Some(resolved) => resolved.clone(),
                        None => remote::discover(&source, &request.selector, options)?,
                    }) else {
                        bail!("invalid release discovery")
                    };
                    let selection = Selection {
                        platform: request.asset.platform.clone(),
                        filters: request.asset.filters.clone(),
                        selection_policy: request.asset.selection_policy.clone(),
                        ..Default::default()
                    };
                    let asset = select_asset(&assets, &selection)?.clone();
                    let expected = expected_digest(request, asset.digest.as_deref())?;
                    (
                        Some(Resolved::Release {
                            id,
                            tag,
                            assets: vec![asset.clone()],
                        }),
                        asset.url,
                        expected,
                    )
                }
            };
            let mut response = match (&source, &resolved) {
                // The asset API works for private releases; browser URLs do not.
                (Source::GithubRelease { repo }, Some(Resolved::Release { assets, .. })) => {
                    let auth = remote::Auth::github(options);
                    let base = remote::github_api(options)?;
                    auth.call(
                        auth.get(&format!(
                            "{base}/repos/{repo}/releases/assets/{}",
                            assets[0].id
                        ))
                        .header("Accept", "application/octet-stream"),
                    )?
                }
                _ => remote::agent()
                    .get(&url)
                    .header("User-Agent", "saucepan")
                    .call()?,
            };
            let resolved = resolved.unwrap_or_else(|| Resolved::Url {
                url: url.clone(),
                etag: response
                    .headers()
                    .get("etag")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
                last_modified: response
                    .headers()
                    .get("last-modified")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
            });
            let path = work.join("material");
            let mut output = fs::File::create(&path)?;
            let transferred = copy_bounded(
                response.body_mut().as_reader(),
                &mut output,
                TreeLimits::default().bytes,
                expected.is_some(),
            )?;
            let verification = verify_digest(expected.as_deref(), transferred.sha256.as_deref())?;
            Ok(Material {
                path,
                kind: MaterialKind::File,
                resolved,
                dependencies: BTreeMap::new(),
                verification,
            })
        }
        Source::GitRepo { .. } | Source::GithubRepo { .. } => unreachable!("repository source"),
        source @ (Source::GithubRaw { .. }
        | Source::GithubSource { .. }
        | Source::GitRaw { .. }) => {
            let resolved = match exact {
                Some(resolved) => resolved.clone(),
                None => remote::discover(&source, &request.selector, options)?,
            };
            let mut response = match (&source, &resolved) {
                (Source::GithubRaw { repo, path }, Resolved::Git { commit }) => {
                    let auth = remote::Auth::github(options);
                    auth.call(auth.get(&remote::github_raw_url(repo, path, commit, options)?))?
                }
                // Snapshot ZIPs carry no history; no bare repository is created.
                (Source::GithubSource { repo }, Resolved::Git { commit }) => {
                    let auth = remote::Auth::github(options);
                    let base = remote::github_api(options)?;
                    auth.call(auth.get(&format!("{base}/repos/{repo}/zipball/{commit}")))?
                }
                (
                    Source::GitRaw {
                        platform,
                        token_file,
                        ..
                    },
                    Resolved::Url { url, .. },
                ) => {
                    let auth = remote::Auth::platform(*platform, token_file.as_deref())?;
                    auth.call(auth.get(url))?
                }
                _ => bail!("invalid provider provenance"),
            };
            let expected = expected_digest(request, None)?;
            let path = work.join("material");
            let transferred = copy_bounded(
                response.body_mut().as_reader(),
                &mut fs::File::create(&path)?,
                TreeLimits::default().bytes,
                expected.is_some(),
            )?;
            Ok(Material {
                path,
                kind: MaterialKind::File,
                resolved,
                dependencies: BTreeMap::new(),
                verification: verify_digest(expected.as_deref(), transferred.sha256.as_deref())?,
            })
        }
    }
}

pub(crate) fn expected_digest(
    request: &RetrievalPlan,
    upstream: Option<&str>,
) -> Result<Option<String>> {
    if request.verification == PayloadVerification::None {
        return Ok(None);
    }
    let upstream = upstream
        .map(|value| {
            value.strip_prefix("sha256:").ok_or_else(|| {
                Error::new(
                    ErrorCode::InvalidRequest,
                    "unsupported expected digest algorithm",
                )
            })
        })
        .transpose()?;
    if let (Some(explicit), Some(upstream)) = (&request.expected_sha256, upstream) {
        ensure!(
            explicit.eq_ignore_ascii_case(upstream),
            Error::new(ErrorCode::IntegrityError, "conflicting expected digests")
        );
    }
    let expected = request.expected_sha256.as_deref().or(upstream);
    ensure!(
        request.verification != PayloadVerification::Required || expected.is_some(),
        Error::new(
            ErrorCode::InvalidRequest,
            "required verification needs an expected digest"
        )
    );
    if let Some(expected) = expected {
        ensure!(
            expected.len() == 64 && expected.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid expected SHA-256"
        );
    }
    Ok(expected.map(str::to_ascii_lowercase))
}

pub(crate) fn verify_digest(
    expected: Option<&str>,
    actual: Option<&str>,
) -> Result<VerificationEvidence> {
    if let Some(expected) = expected {
        ensure!(
            actual.is_some_and(|actual| actual.eq_ignore_ascii_case(expected)),
            Error::new(ErrorCode::IntegrityError, "payload digest mismatch")
        );
    }
    Ok(VerificationEvidence {
        expected_sha256: expected.map(str::to_owned),
        ..Default::default()
    })
}
