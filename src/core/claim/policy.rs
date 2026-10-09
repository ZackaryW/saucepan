//! Pure permission evaluation. The session boundary must verify proof and select
//! authority; these inputs must never be deserialized from an app request.
use crate::{
    error::{Error, ErrorCode, Result},
    models::claim::{
        Scope,
        policy::{AccessPolicy, Action},
    },
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum Proof {
    Missing,
    Invalid,
    Verified,
}
#[derive(Debug, Clone, Copy)]
pub enum Caller {
    Application {
        app: Uuid,
        proof: Proof,
    },
    PublicDiscovery,
    /// Explicit trusted local administration, never an app-selectable request field.
    Administration {
        proof: Proof,
    },
}
pub struct Resource<'a> {
    pub app: Uuid,
    pub source: Option<Uuid>,
    pub scope: Option<&'a Scope>,
}

pub fn authorize(
    policy: &AccessPolicy,
    credentials_required: bool,
    caller: Caller,
    action: Action,
    resource: &Resource<'_>,
) -> Result<()> {
    let deny = || Error::new(ErrorCode::AccessDenied, "operation is not permitted");
    let proof = match caller {
        Caller::Application { app, proof } => {
            if app != resource.app {
                return Err(deny());
            }
            proof
        }
        Caller::PublicDiscovery => {
            if action != Action::Discover
                || credentials_required
                || !resource
                    .scope
                    .is_some_and(|s| policy.public_scopes.contains(s))
            {
                return Err(deny());
            }
            Proof::Missing
        }
        Caller::Administration { proof } => proof,
    };
    if credentials_required {
        match proof {
            Proof::Missing => {
                return Err(Error::new(
                    ErrorCode::AuthenticationRequired,
                    "application credential required",
                ));
            }
            Proof::Invalid => {
                return Err(Error::new(
                    ErrorCode::AuthenticationFailed,
                    "application credential rejected",
                ));
            }
            Proof::Verified => (),
        }
    }
    if matches!(caller, Caller::Administration { .. }) {
        return Ok(());
    }
    if action == Action::RegisterPolicy {
        return Err(deny());
    }
    if policy
        .actions
        .as_ref()
        .is_some_and(|allowed| !allowed.contains(&action))
        || policy
            .sources
            .as_ref()
            .is_some_and(|allowed| !resource.source.is_some_and(|id| allowed.contains(&id)))
        || policy
            .scopes
            .as_ref()
            .is_some_and(|allowed| !resource.scope.is_some_and(|id| allowed.contains(id)))
    {
        return Err(deny());
    }
    Ok(())
}
