use crate::core::Store;
use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(clap::Subcommand)]
pub(super) enum PolicyCommand {
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
pub(super) fn run(
    store: &Store,
    command: PolicyCommand,
    credential_file: Option<&Path>,
) -> Result<serde_json::Value> {
    Ok(match command {
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
            store.adopt_application_policies(
                app,
                expected_revision,
                policies.try_into()?,
                proof.as_ref(),
            )?;
            let registry = store.registry(app)?.registry;
            serde_json::json!({"app":app,"revision":registry.revision,"policy_refs":registry.policy_refs})
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
        } => {
            store.adopt_claim_policies(app, claim, expected_revision, policies.try_into()?)?;
            serde_json::to_value(&store.registry(app)?.claims[&claim])?
        }
    })
}
