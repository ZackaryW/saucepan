use crate::{
    error::{Error, ErrorCode, Result},
    models::{
        policy::{Freshness, PayloadVerification, RevisionPolicy},
        retrieval::{CachePolicy, RetrievalPlan},
        store::{AcquisitionPlan, Destination},
    },
    utils::path::relative_path,
};

fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InvalidRequest, message)
}

/// Pure preflight; does not resolve paths, open a store or contact a provider.
pub fn validate_retrieval(request: &RetrievalPlan) -> Result<()> {
    if let Some(hash) = &request.expected_sha256 {
        if request.verification == PayloadVerification::None {
            return Err(invalid(
                "none verification conflicts with an expected digest",
            ));
        }
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid("invalid expected SHA-256"));
        }
    }
    if let CachePolicy::NoStore { destination } = &request.cache
        && destination.as_os_str().is_empty()
    {
        return Err(invalid("no_store requires a destination"));
    }
    for filter in &request.asset.filters {
        for pattern in &filter.asset_patterns {
            globset::Glob::new(pattern).map_err(|_| invalid("invalid asset pattern"))?;
        }
    }
    Ok(())
}

/// Context-free compatibility check; destination existence and permissions are
/// validated by the operation before it creates staging or contacts a provider.
pub fn validate_composition(
    retrieval: &RetrievalPlan,
    acquisition: &AcquisitionPlan,
    update_due: bool,
) -> Result<()> {
    validate_retrieval(retrieval)?;
    match &acquisition.destination {
        Destination::Managed {} => {
            if !matches!(retrieval.cache, CachePolicy::Cached { .. }) {
                return Err(invalid("managed output requires cached retrieval"));
            }
            acquisition.revision_policy.unwrap_or_default().keep()?;
        }
        destination => {
            if acquisition.revision_policy.is_some() || acquisition.protection.is_some() {
                return Err(invalid("revision policy requires managed output"));
            }
            if let Destination::Directory { path } = destination
                && path.as_os_str().is_empty()
            {
                return Err(invalid("caller-owned output requires a destination"));
            }
        }
    }
    if update_due
        && matches!(
            retrieval.cache,
            CachePolicy::Cached {
                freshness: Some(Freshness::CacheOnly {}),
                ..
            }
        )
    {
        return Err(invalid("due update conflicts with cache_only retrieval"));
    }
    for path in acquisition
        .folder
        .iter()
        .chain(acquisition.filename.iter())
        .chain(&acquisition.bin)
    {
        relative_path(path).map_err(|_| invalid("invalid acquisition path"))?;
    }
    if acquisition
        .filename
        .as_ref()
        .is_some_and(|name| name.contains('/'))
    {
        return Err(invalid("filename must be one component"));
    }
    Ok(())
}

pub fn effective_revision(request: &AcquisitionPlan) -> Result<Option<RevisionPolicy>> {
    if matches!(request.destination, Destination::Managed {}) {
        let policy = request.revision_policy.unwrap_or_default();
        policy.keep()?;
        Ok(Some(policy))
    } else if request.revision_policy.is_some() {
        Err(invalid("revision policy requires managed output"))
    } else {
        Ok(None)
    }
}
