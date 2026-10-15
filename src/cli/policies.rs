use crate::core::Store;
use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(clap::Subcommand)]
pub(super) enum PolicyCommand {
    /// Explain a typed request without provider work or adoption.
    ExplainPolicies { file: PathBuf },
    /// List built-in profiles without opening a store.
    Profiles,
    /// Expand a recipe into ordinary immutable policy definitions.
    Profile {
        name: Option<String>,
        #[arg(long)]
        for_install: bool,
        #[arg(long)]
        catalog: Option<PathBuf>,
        #[arg(long = "retrieval-policy")]
        retrieval: Vec<uuid::Uuid>,
        #[arg(long = "acquisition-policy")]
        acquisition: Vec<uuid::Uuid>,
    },
    /// Register an array of immutable policy definitions from JSON.
    RegisterPolicies { file: PathBuf },
    /// Inspect one policy definition by UUID; no app or query file is required.
    #[command(
        after_help = "Examples:\n  saucepan policy POLICY_UUID\n  saucepan --root ./scratch policy POLICY_UUID\n  saucepan --as-app APP_UUID policy POLICY_UUID"
    )]
    Policy { uuid: String },
    /// Replace an application's policy references (trusted administration).
    AdoptAppPolicies {
        app: uuid::Uuid,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long = "policy")]
        policies: Vec<uuid::Uuid>,
    },
    /// Replace a source's policy references (trusted administration).
    AdoptSourcePolicies {
        source: uuid::Uuid,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long = "policy")]
        policies: Vec<uuid::Uuid>,
    },
    /// Adopt permitted behavior for a claim at its application's revision.
    AdoptClaimPolicies {
        app: uuid::Uuid,
        claim: uuid::Uuid,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long = "policy")]
        policies: Vec<uuid::Uuid>,
    },
}
/// Root-free preview must be dispatched before store resolution.
pub(super) fn preview(command: &PolicyCommand) -> Result<Option<serde_json::Value>> {
    use crate::{core::policy::profile, models::policy::profile::ProfileRequest};
    Ok(Some(match command {
        PolicyCommand::Profiles => serde_json::to_value(profile::NAMES)?,
        PolicyCommand::Profile {
            name,
            for_install,
            catalog,
            retrieval,
            acquisition,
        } => {
            let name = serde_json::from_value(serde_json::Value::String(
                name.as_deref().unwrap_or("managed").into(),
            ))
            .map_err(|_| {
                crate::error::Error::new(crate::error::ErrorCode::InvalidRequest, "unknown profile")
            })?;
            let catalog = catalog
                .as_ref()
                .map(|path| -> Result<_> { Ok(serde_json::from_slice(&std::fs::read(path)?)?) })
                .transpose()?
                .unwrap_or_default();
            serde_json::to_value(profile::expand(&ProfileRequest {
                name,
                for_install: *for_install,
                catalog,
                retrieval_policy_refs: retrieval.clone().try_into()?,
                acquisition_policy_refs: acquisition.clone().try_into()?,
            })?)?
        }
        _ => return Ok(None),
    }))
}
impl PolicyCommand {
    pub fn application(&self) -> Option<uuid::Uuid> {
        match self {
            Self::AdoptClaimPolicies { app, .. } => Some(*app),
            _ => None,
        }
    }
    pub fn administration(&self) -> bool {
        matches!(
            self,
            Self::RegisterPolicies { .. }
                | Self::AdoptAppPolicies { .. }
                | Self::AdoptSourcePolicies { .. }
        )
    }
}
pub(super) fn explain(file: &Path, args: &super::Args) -> Result<()> {
    use crate::{
        core::policy::{catalog::PolicyCatalog, explain},
        models::{policy::explanation::ExplainRequest, retrieval::SourceInput},
    };
    let input: ExplainRequest = serde_json::from_slice(&std::fs::read(file)?)?;
    let portable = if let Some(path) = &args.portable_policies {
        PolicyCatalog::new(serde_json::from_slice::<
            Vec<crate::models::policy::PolicyRecord>,
        >(&std::fs::read(path)?)?)?
    } else {
        PolicyCatalog::default()
    };
    let target = match &input {
        ExplainRequest::Claim { app, .. } => Some(*app),
        _ => None,
    };
    anyhow::ensure!(
        args.as_app.is_none() || target.is_none() || args.as_app == target,
        crate::error::Error::new(
            crate::error::ErrorCode::AccessDenied,
            "application identity mismatch"
        )
    );
    let app = args.as_app.or(target);
    anyhow::ensure!(
        args.credential_file.is_none() || app.is_some(),
        crate::error::Error::new(
            crate::error::ErrorCode::InvalidRequest,
            "credential-file requires application context"
        )
    );
    let value = if app.is_none() && matches!(input.source(), Some(SourceInput::Inline { .. })) {
        explain::portable(&input, &portable)?
    } else {
        let root = super::root::resolve(args.root.clone())?;
        super::root::require_existing(&root, args.root.is_some())?;
        let mut store = Store::open_metadata(&root, &crate::core::claim::security::NativeKeyring)?;
        if let Some(app) = app {
            store = store
                .session(
                    app,
                    super::credentials::read(args.credential_file.as_deref())?,
                )?
                .into_store();
        }
        store.explain_policies(&input, &portable)?
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

pub(super) fn run(
    store: &Store,
    command: PolicyCommand,
    credential_file: Option<&Path>,
) -> Result<serde_json::Value> {
    Ok(match command {
        PolicyCommand::Profiles
        | PolicyCommand::Profile { .. }
        | PolicyCommand::ExplainPolicies { .. } => {
            unreachable!("root-free preview")
        }
        PolicyCommand::RegisterPolicies { file } => {
            serde_json::to_value(store.register_policies(&serde_json::from_slice::<
                Vec<crate::models::policy::PolicyRecord>,
            >(&std::fs::read(
                file,
            )?)?)?)?
        }
        PolicyCommand::Policy { uuid } => {
            let uuid = uuid::Uuid::parse_str(&uuid).map_err(|_| {
                crate::error::Error::new(
                    crate::error::ErrorCode::InvalidRequest,
                    "malformed policy UUID",
                )
            })?;
            serde_json::to_value(store.policy(uuid)?)?
        }
        PolicyCommand::AdoptAppPolicies {
            app,
            expected_revision,
            policies,
        } => {
            let proof = super::credentials::read(credential_file)?;
            serde_json::to_value(store.adopt_application_policies(
                app,
                expected_revision,
                policies.try_into()?,
                proof.as_ref(),
            )?)?
        }
        PolicyCommand::AdoptSourcePolicies {
            source,
            expected_revision,
            policies,
        } => serde_json::to_value(store.adopt_source_policies(
            source,
            expected_revision,
            policies.try_into()?,
        )?)?,
        PolicyCommand::AdoptClaimPolicies {
            app,
            claim,
            expected_revision,
            policies,
        } => serde_json::to_value(store.adopt_claim_policies(
            app,
            claim,
            expected_revision,
            policies.try_into()?,
        )?)?,
    })
}
