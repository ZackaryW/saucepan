use crate::{models::store::OutputPlan, utils::path::relative_path};
use anyhow::{Result, ensure};
pub(crate) mod access;
pub(crate) mod adopt;
pub(crate) mod authority;
pub mod catalog;
pub(crate) mod claims;
pub mod composition;
mod execution;
pub mod fence;
pub mod group;
pub(crate) mod persistence;
pub mod prepare;
pub mod profile;
pub mod resolve;
pub(crate) mod source;
pub fn validate(request: &OutputPlan) -> Result<()> {
    let keep = request.policies.revision.keep()?;
    ensure!(keep > 0, "retention must be positive");
    for p in request
        .selection
        .folder
        .iter()
        .chain(request.selection.filename.iter())
        .chain(&request.selection.bin)
    {
        relative_path(p)?;
    }
    if let Some(name) = &request.selection.filename {
        ensure!(!name.contains('/'), "filename must be one component");
    }
    for filter in &request.selection.filters {
        for pattern in &filter.asset_patterns {
            globset::Glob::new(pattern)?;
        }
    }
    if let Some(hash) = &request.selection.sha256 {
        ensure!(
            hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid SHA-256"
        );
    }
    Ok(())
}

pub fn select_asset<'a>(
    assets: &'a [crate::models::source::Asset],
    selection: &crate::models::store::Selection,
) -> Result<&'a crate::models::source::Asset> {
    let platform = selection
        .platform
        .clone()
        .unwrap_or_else(crate::models::store::Platform::host);
    for filter in &selection.filters {
        if filter.platform.as_ref().is_some_and(|p| *p != platform) {
            continue;
        }
        let patterns = filter
            .asset_patterns
            .iter()
            .map(|p| globset::Glob::new(p).map(|g| g.compile_matcher()))
            .collect::<Result<Vec<_>, _>>()?;
        let matches: Vec<_> = assets
            .iter()
            .filter(|a| patterns.iter().any(|p| p.is_match(&a.name)))
            .collect();
        if matches.is_empty() {
            continue;
        }
        ensure!(
            matches.len() == 1,
            "asset filter is ambiguous: {} matches",
            matches.len()
        );
        return Ok(matches[0]);
    }
    anyhow::bail!("no release asset matches platform and filters")
}
mod output;
