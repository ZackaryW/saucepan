use crate::{
    core::Store,
    error::{Error, ErrorCode},
    models::{
        claim::{Claim, RegistryState, Scope, policy::Effective},
        source::Source,
        store::{Format, Request},
    },
};
use anyhow::{Result, ensure};

pub(super) struct Match {
    pub claim: Claim,
    pub relative: Option<String>,
}

/// Match ownership only within an explicit scope. Context does not form identity.
pub(super) fn select(
    store: &Store,
    registry: &RegistryState,
    scope: &Scope,
    request: &Request,
    effective: &Effective,
) -> Result<Option<Match>> {
    let mut matches = Vec::new();
    let mut overlap = false;
    for claim in registry.claims.values().filter(|claim| {
        &claim.scope == scope
            && claim.request.source_uuid == request.source_uuid
            && claim.request.selector == request.selector
    }) {
        let mut left = claim.request.selection.clone();
        let mut right = request.selection.clone();
        left.folder = None;
        right.folder = None;
        if left != right {
            continue;
        }
        let exact = claim.request.selection.folder == request.selection.folder;
        let covers = projection(
            claim.request.selection.folder.as_deref(),
            request.selection.folder.as_deref(),
        );
        let reverse = projection(
            request.selection.folder.as_deref(),
            claim.request.selection.folder.as_deref(),
        );
        if !exact {
            if request.selection.format != Format::Natural {
                continue;
            }
            if !matches!(
                store.source(request.source_uuid)?.source,
                Source::Local { .. } | Source::Git { .. } | Source::GithubRepo { .. }
            ) {
                continue;
            }
            if covers.is_none() && reverse.is_none() {
                continue;
            }
        }
        // Reuse/new_entry chooses acquisition behavior, not a second ownership
        // identity. Retention, protection and recovery must still agree.
        let mut compatible = claim.effective.clone();
        compatible.output.identical = effective.output.identical;
        if &compatible != effective {
            overlap = true;
            continue;
        }
        if exact {
            matches.push((
                usize::MAX,
                Match {
                    claim: claim.clone(),
                    relative: None,
                },
            ));
        } else if let Some(relative) = covers {
            let output = store.output(claim.entry_uuid)?;
            ensure!(
                output
                    .files
                    .get(&relative)
                    .is_some_and(|entry| entry.digest.is_none()),
                Error::new(
                    ErrorCode::InvalidRequest,
                    "requested folder is absent from the owning output manifest"
                )
            );
            ensure!(
                request.selection.bin.iter().all(|bin| output
                    .files
                    .get(&format!("{relative}/{bin}"))
                    .is_some_and(|entry| entry.digest.is_some())),
                Error::new(
                    ErrorCode::InvalidRequest,
                    "requested binary is absent from the covered folder manifest"
                )
            );
            let specificity = claim
                .request
                .selection
                .folder
                .as_ref()
                .map_or(0, |folder| folder.split('/').count());
            matches.push((
                specificity,
                Match {
                    claim: claim.clone(),
                    relative: Some(relative),
                },
            ));
        } else {
            overlap = true;
        }
    }
    ensure!(
        !overlap,
        Error::new(
            ErrorCode::ClaimOverlap,
            "request overlaps existing scoped ownership; release or reorganize claims explicitly"
        )
    );
    matches.sort_by(|(a, left), (b, right)| {
        b.cmp(a)
            .then_with(|| left.claim.uuid.cmp(&right.claim.uuid))
    });
    Ok(matches.into_iter().next().map(|(_, matched)| matched))
}

fn projection(parent: Option<&str>, child: Option<&str>) -> Option<String> {
    match (parent, child) {
        (None, Some(child)) => Some(child.to_owned()),
        (Some(parent), Some(child)) => child
            .strip_prefix(parent)
            .and_then(|suffix| suffix.strip_prefix('/'))
            .map(str::to_owned),
        _ => None,
    }
}
