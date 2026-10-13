//! Test builders register explicit leaf definitions before constructing inputs.
#![allow(dead_code)]
use anyhow::Result;
use saucepan::{
    core::Store,
    models::{
        claim::policy::{AccessPolicy, Effective, ReclaimPolicy, ReclaimStrategy},
        policy::{Identical, PolicyRecord, PolicyRefs, RevisionPolicy, UpdatePolicy},
    },
};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub defaults: Effective,
    pub minimum_keep: usize,
    pub maximum_keep: usize,
    pub allowed_identical: Vec<Identical>,
    pub allowed_reclaim: Vec<ReclaimStrategy>,
    pub allow_useable_app: bool,
    pub allow_automatic_updates: bool,
    pub minimum_update_interval: u64,
}
impl Default for Registration {
    fn default() -> Self {
        Self {
            defaults: Effective::default(),
            minimum_keep: 1,
            maximum_keep: usize::MAX,
            allowed_identical: vec![Identical::Reuse, Identical::NewEntry],
            allowed_reclaim: vec![
                ReclaimStrategy::ReportUnavailable,
                ReclaimStrategy::RestoreExact,
                ReclaimStrategy::Refresh,
            ],
            allow_useable_app: true,
            allow_automatic_updates: true,
            minimum_update_interval: 0,
        }
    }
}

pub fn register(store: &Store, access: AccessPolicy, settings: Registration) -> Result<PolicyRefs> {
    let mut revision = serde_json::to_value(settings.defaults.output.revision)?;
    revision["type"] = json!("revision_policy");
    let mut values = vec![
        json!({"type":"access_policy","sources":access.sources,"scopes":access.scopes,"actions":access.actions}),
        json!({"type":"claim_visibility_policy","public_scopes":access.public_scopes}),
        json!({"type":"identical_pull_policy","strategy":settings.defaults.output.identical}),
        revision,
        json!({"type":"reclaim_policy","strategy":settings.defaults.reclaim.strategy()}),
        json!({"type":"policy_constraints","minimum_keep":settings.minimum_keep,"maximum_keep":settings.maximum_keep,
            "allowed_identical":settings.allowed_identical,"allowed_reclaim":settings.allowed_reclaim,
            "allow_useable_app":settings.allow_useable_app,"require_useable_app":settings.defaults.output.protection.is_some(),
            "allow_automatic_updates":settings.allow_automatic_updates,"minimum_update_interval":settings.minimum_update_interval}),
    ];
    let mut update = serde_json::to_value(settings.defaults.update_policy)?;
    update["type"] = json!("update_policy");
    values.push(update);
    if settings.defaults.output.protection.is_some() {
        values.push(json!({"type":"useable_app_policy"}));
    }
    let records = records(values)?;
    let catalog = saucepan::core::policy::catalog::PolicyCatalog::new(records.clone())?;
    let refs: PolicyRefs = records
        .iter()
        .map(|r| r.uuid)
        .collect::<Vec<_>>()
        .try_into()?;
    let empty = PolicyRefs::default();
    saucepan::core::policy::resolve::evaluate(
        &catalog,
        saucepan::core::policy::resolve::Inputs {
            source: &empty,
            application: &refs,
            request: &empty,
            pinned: None,
            operation: saucepan::core::policy::resolve::Operation::Compose,
            managed_output: true,
            update_due: false,
        },
    )?;
    Ok(store.register_policies(&records)?.try_into()?)
}

#[derive(Default)]
pub struct Overrides {
    pub identical: Option<Identical>,
    pub revision: Option<RevisionPolicy>,
    pub reclaim: Option<ReclaimPolicy>,
    /// Output protection is registered by the app; overrides cannot remove it.
    pub useable_app: Option<bool>,
    pub update_policy: Option<UpdatePolicy>,
}

pub fn requested(store: &Store, settings: Overrides) -> Result<PolicyRefs> {
    let mut values = Vec::new();
    if let Some(value) = settings.identical {
        values.push(json!({"type":"identical_pull_policy","strategy":value}));
    }
    if let Some(value) = settings.reclaim {
        values.push(json!({"type":"reclaim_policy","strategy":value.strategy()}));
    }
    for (kind, value) in [
        (
            "revision_policy",
            settings.revision.map(serde_json::to_value),
        ),
        (
            "update_policy",
            settings.update_policy.map(serde_json::to_value),
        ),
    ] {
        if let Some(value) = value {
            let mut value = value?;
            value["type"] = json!(kind);
            values.push(value);
        }
    }
    if settings.useable_app == Some(true) {
        values.push(json!({"type":"useable_app_policy"}));
    }
    anyhow::ensure!(
        settings.useable_app != Some(false),
        "no policy disables required protection"
    );
    definitions(store, values)
}

fn records(values: Vec<Value>) -> Result<Vec<PolicyRecord>> {
    values
        .into_iter()
        .map(|mut value| -> Result<_> {
            let digest = saucepan::utils::tree::digest(serde_json::to_vec(&value)?.as_slice())?;
            // Deterministic fixture identities keep registration retries meaningful.
            let id = Uuid::parse_str(&digest[..32])?;
            value["uuid"] = Value::String(id.to_string());
            Ok(serde_json::from_value(value)?)
        })
        .collect()
}

pub fn definitions(store: &Store, values: Vec<Value>) -> Result<PolicyRefs> {
    Ok(store.register_policies(&records(values)?)?.try_into()?)
}

pub fn replace(store: &Store, refs: &mut PolicyRefs, value: Value) -> Result<()> {
    let next = definitions(store, vec![value])?;
    let id = next.ids()[0];
    let axis = store.policy(id)?.definition.axis();
    let mut retained = Vec::new();
    for existing in refs.ids() {
        if store.policy(*existing)?.definition.axis() != axis {
            retained.push(*existing);
        }
    }
    retained.push(id);
    *refs = retained.try_into()?;
    Ok(())
}
