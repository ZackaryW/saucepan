//! Pure built-in recipes. Identity constants are immutable: edits need new UUIDs.
use super::{catalog::PolicyCatalog, resolve::Operation};
use crate::{
    error::{Error, ErrorCode, Result},
    models::{
        claim::policy::ReclaimStrategy,
        policy::{
            Freshness, Identical, PolicyDefinition as Def, PolicyRecord, PolicyRefs,
            RevisionPolicy, UpdatePolicy,
            profile::{ProfileExpansion, ProfileName, ProfileRequest},
            record::{Axis, CacheMode},
        },
    },
    utils::tree::digest,
};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub const NAMES: [ProfileName; 3] = [
    ProfileName::Managed,
    ProfileName::ExplicitUpdate,
    ProfileName::Temporary,
];

fn builtins() -> Vec<PolicyRecord> {
    [
        (
            1,
            Def::CachePolicy {
                mode: CacheMode::Cached,
            },
        ),
        (
            2,
            Def::CachePolicy {
                mode: CacheMode::Temporary,
            },
        ),
        (3, Def::RefreshPolicy(Freshness::MaxAge { seconds: 86_400 })),
        (4, Def::RefreshPolicy(Freshness::Always {})),
        (5, Def::StalePolicy { allow_stale: false }),
        (6, Def::UpdatePolicy(UpdatePolicy::Manual {})),
        (
            7,
            Def::IdenticalPullPolicy {
                strategy: Identical::Reuse,
            },
        ),
        (8, Def::RevisionPolicy(RevisionPolicy::Latest { keep: 10 })),
        (
            9,
            Def::ReclaimPolicy {
                strategy: ReclaimStrategy::ReportUnavailable,
            },
        ),
        (10, Def::UseableAppPolicy {}),
    ]
    .into_iter()
    .map(|(id, definition)| PolicyRecord {
        uuid: identity(id),
        definition,
    })
    .collect()
}

fn identity(id: u128) -> Uuid {
    Uuid::from_u128(0xacf53062_66cd_4dd9_94bb_c9f849eb0000 | id)
}

fn merge(
    catalog: &PolicyCatalog,
    base: &[u128],
    overrides: &PolicyRefs,
    operation: Operation,
) -> Result<PolicyRefs> {
    let mut chosen: BTreeMap<Axis, Uuid> = base
        .iter()
        .map(|id| {
            let record = catalog.get(identity(*id))?;
            Ok((
                record.definition.axis().expect("builtin singleton"),
                record.uuid,
            ))
        })
        .collect::<Result<_>>()?;
    let mut explicit = BTreeSet::new();
    for id in overrides.ids() {
        let record = catalog.get(*id)?;
        let axis = record
            .definition
            .axis()
            .filter(|axis| operation.accepts(*axis))
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::InvalidRequest,
                    "profile override targets another operation",
                )
            })?;
        if !explicit.insert(axis) {
            return Err(Error::new(
                ErrorCode::PolicyConflict,
                "competing profile overrides",
            ));
        }
        chosen.insert(axis, *id);
    }
    chosen.into_values().collect::<Vec<_>>().try_into()
}

pub fn expand(request: &ProfileRequest) -> Result<ProfileExpansion> {
    if request.for_install && request.name == ProfileName::Temporary {
        return Err(Error::new(
            ErrorCode::PolicyConflict,
            "temporary profile cannot install an application",
        ));
    }
    let custom = PolicyCatalog::new(request.catalog.clone())?;
    let mut records = builtins();
    for record in custom.records() {
        if let Some(existing) = records.iter().find(|r| r.uuid == record.uuid) {
            if existing != record {
                return Err(Error::new(
                    ErrorCode::PolicyConflict,
                    "profile definition identity conflict",
                ));
            }
        } else {
            records.push(record.clone());
        }
    }
    let catalog = PolicyCatalog::new(records)?;
    let retrieval = match request.name {
        ProfileName::Managed => [1, 3, 5],
        ProfileName::ExplicitUpdate => [1, 4, 5],
        ProfileName::Temporary => [2, 4, 5],
    };
    let mut acquisition = if request.name == ProfileName::Temporary {
        vec![]
    } else {
        vec![6, 7, 8, 9]
    };
    if request.for_install {
        acquisition.push(10);
    }
    let retrieval_policy_refs = merge(
        &catalog,
        &retrieval,
        &request.retrieval_policy_refs,
        Operation::Retrieve,
    )?;
    let acquisition_policy_refs = merge(
        &catalog,
        &acquisition,
        &request.acquisition_policy_refs,
        Operation::Acquire,
    )?;
    if request.for_install {
        let cache = retrieval_policy_refs.ids().iter().find_map(|id| {
            match catalog.get(*id).ok()?.definition {
                Def::CachePolicy { mode } => Some(mode),
                _ => None,
            }
        });
        if cache != Some(CacheMode::Cached) {
            return Err(Error::new(
                ErrorCode::PolicyConflict,
                "installation requires cached retrieval",
            ));
        }
    }
    let ids: BTreeSet<_> = retrieval_policy_refs
        .ids()
        .iter()
        .chain(acquisition_policy_refs.ids())
        .copied()
        .collect();
    let records = ids
        .iter()
        .map(|id| catalog.get(*id).cloned())
        .collect::<Result<Vec<_>>>()?;
    let bytes = serde_json::to_vec(&(request.name, request.for_install, &records))
        .map_err(|_| Error::new(ErrorCode::InvalidRequest, "cannot encode profile"))?;
    Ok(ProfileExpansion {
        name: request.name,
        for_install: request.for_install,
        digest: digest(bytes.as_slice())?,
        catalog: records,
        retrieval_policy_refs,
        acquisition_policy_refs,
        combined_policy_refs: ids.into_iter().collect::<Vec<_>>().try_into()?,
    })
}
