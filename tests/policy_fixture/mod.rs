//! Test builders register explicit leaf definitions before constructing inputs.
#![allow(dead_code)]
use anyhow::Result;
use saucepan::{
    core::Store,
    models::{
        claim::policy::{AccessPolicy, Registration},
        policy::{PolicyRecord, PolicyRefs},
    },
};
use serde_json::{Value, json};
use uuid::Uuid;

pub fn register(store: &Store, access: AccessPolicy, settings: Registration) -> Result<PolicyRefs> {
    settings.validate()?;
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
    let records: Vec<PolicyRecord> = values
        .into_iter()
        .map(|mut value| -> Result<_> {
            let digest = saucepan::utils::tree::digest(serde_json::to_vec(&value)?.as_slice())?;
            // Deterministic fixture identities keep registration retries meaningful.
            let id = Uuid::parse_str(&digest[..32])?;
            value["uuid"] = Value::String(id.to_string());
            Ok(serde_json::from_value(value)?)
        })
        .collect::<Result<_>>()?;
    Ok(store.register_policies(&records)?.try_into()?)
}
