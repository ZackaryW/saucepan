use saucepan::{
    core::claim::policy::{Caller, Proof, Resource, authorize},
    error::{Error, ErrorCode},
    models::claim::{
        CoveredView, Scope,
        policy::{AccessPolicy, Action, Overrides, ReclaimPolicy, Registration},
        security::{Credential, CredentialPolicy},
    },
};
use serde_json::json;
use std::collections::BTreeSet;
use uuid::Uuid;

fn scope(s: &str) -> Scope {
    s.to_owned().try_into().unwrap()
}

#[test]
fn scope_and_view_contracts_preserve_labels_but_reject_unsafe_projections() {
    for label in ["C:\\work\\a", "scope.*", " project ", "é"] {
        let s = scope(label);
        assert_eq!(s.as_str(), label);
        assert_eq!(serde_json::from_value::<Scope>(json!(label)).unwrap(), s);
    }
    for label in [String::new(), "é".repeat(2049)] {
        assert!(Scope::try_from(label.clone()).is_err());
        assert!(serde_json::from_value::<Scope>(json!(label)).is_err());
    }
    assert!(Scope::try_from("é".repeat(2048)).is_ok());
    let mut view = CoveredView {
        app: Uuid::new_v4(),
        claim: Uuid::new_v4(),
        entry: Uuid::new_v4(),
        relative: None,
    };
    assert!(view.validate().is_ok());
    for path in ["../escape", "/absolute", "C:\\outside", "a/../b", ""] {
        view.relative = Some(path.into());
        assert!(view.validate().is_err());
    }
    view.relative = Some("skills/core".into());
    assert!(view.validate().is_ok());
}

#[test]
fn omitted_permissions_and_empty_permissions_have_different_meanings() {
    let unrestricted: AccessPolicy = serde_json::from_value(json!({})).unwrap();
    let empty: AccessPolicy =
        serde_json::from_value(json!({"sources":[],"scopes":[],"actions":[]})).unwrap();
    assert!(unrestricted.sources.is_none());
    assert_eq!(empty.sources, Some(BTreeSet::new()));
    assert!(unrestricted.public_scopes.is_empty());
    assert!(serde_json::from_value::<AccessPolicy>(json!({"allow_everything":true})).is_err());
    assert!(serde_json::from_value::<AccessPolicy>(json!({"actions":["invented"]})).is_err());
    assert_eq!(
        serde_json::from_value::<AccessPolicy>(serde_json::to_value(&empty).unwrap()).unwrap(),
        empty
    );
}

#[test]
fn one_evaluator_intersects_permissions_for_every_application_action() {
    let app = Uuid::new_v4();
    let source = Uuid::new_v4();
    let s = scope("project-a");
    let resource = Resource {
        app,
        source: Some(source),
        scope: Some(&s),
    };
    let caller = Caller::Application {
        app,
        proof: Proof::Missing,
    };
    let mut policy = AccessPolicy {
        sources: Some([source].into()),
        scopes: Some([s.clone()].into()),
        ..Default::default()
    };
    for action in [
        Action::Discover,
        Action::Read,
        Action::Acquire,
        Action::Lease,
        Action::Update,
        Action::Recover,
        Action::Release,
        Action::Activate,
        Action::Remove,
        Action::SetContext,
    ] {
        assert!(authorize(&policy, false, caller, action, &resource).is_ok());
        let wrong = Resource {
            source: Some(Uuid::new_v4()),
            ..resource
        };
        assert_eq!(
            authorize(&policy, false, caller, action, &wrong)
                .unwrap_err()
                .code,
            ErrorCode::AccessDenied
        );
        policy.actions = Some(BTreeSet::new());
        assert_eq!(
            authorize(&policy, false, caller, action, &resource)
                .unwrap_err()
                .code,
            ErrorCode::AccessDenied
        );
        policy.actions = None;
    }
    policy.scopes = Some(BTreeSet::new());
    assert!(authorize(&policy, false, caller, Action::Read, &resource).is_err());
    policy.scopes = None;
    policy.sources = Some(BTreeSet::new());
    assert!(authorize(&policy, false, caller, Action::Read, &resource).is_err());
}

#[test]
fn proof_does_not_grant_access_or_policy_escalation() {
    let app = Uuid::new_v4();
    let s = scope("project-a");
    let resource = Resource {
        app,
        source: None,
        scope: Some(&s),
    };
    let policy = AccessPolicy::default();
    for (proof, code) in [
        (Proof::Missing, ErrorCode::AuthenticationRequired),
        (Proof::Invalid, ErrorCode::AuthenticationFailed),
    ] {
        assert_eq!(
            authorize(
                &policy,
                true,
                Caller::Application { app, proof },
                Action::Read,
                &resource
            )
            .unwrap_err()
            .code,
            code
        );
    }
    let verified = Caller::Application {
        app,
        proof: Proof::Verified,
    };
    assert!(authorize(&policy, true, verified, Action::Read, &resource).is_ok());
    assert_eq!(
        authorize(&policy, true, verified, Action::RegisterPolicy, &resource)
            .unwrap_err()
            .code,
        ErrorCode::AccessDenied
    );
    let deny = AccessPolicy {
        actions: Some(BTreeSet::new()),
        ..policy.clone()
    };
    assert!(authorize(&deny, true, verified, Action::Read, &resource).is_err());
    assert!(
        authorize(
            &policy,
            true,
            Caller::Application {
                app: Uuid::new_v4(),
                proof: Proof::Verified
            },
            Action::Read,
            &resource
        )
        .is_err()
    );
    assert!(
        authorize(
            &policy,
            true,
            Caller::Administration {
                proof: Proof::Missing
            },
            Action::RegisterPolicy,
            &resource
        )
        .is_err()
    );
    assert!(
        authorize(
            &policy,
            true,
            Caller::Administration {
                proof: Proof::Verified
            },
            Action::RegisterPolicy,
            &resource
        )
        .is_ok()
    );
}

#[test]
fn public_discovery_is_explicit_and_grants_no_other_action() {
    let s = scope("project-a");
    let resource = Resource {
        app: Uuid::new_v4(),
        source: None,
        scope: Some(&s),
    };
    let mut policy = AccessPolicy::default();
    assert!(
        authorize(
            &policy,
            false,
            Caller::PublicDiscovery,
            Action::Discover,
            &resource
        )
        .is_err()
    );
    policy.public_scopes.insert(s.clone());
    assert!(
        authorize(
            &policy,
            false,
            Caller::PublicDiscovery,
            Action::Discover,
            &resource
        )
        .is_ok()
    );
    assert!(
        authorize(
            &policy,
            false,
            Caller::PublicDiscovery,
            Action::Acquire,
            &resource
        )
        .is_err()
    );
    assert!(
        authorize(
            &policy,
            true,
            Caller::PublicDiscovery,
            Action::Discover,
            &resource
        )
        .is_err()
    );
    policy.scopes = Some(BTreeSet::new());
    assert!(
        authorize(
            &policy,
            false,
            Caller::PublicDiscovery,
            Action::Discover,
            &resource
        )
        .is_err()
    );
}

#[test]
fn registered_policies_apply_defaults_and_reject_out_of_bounds_or_unknown_overrides() {
    let mut registration = Registration {
        maximum_keep: 10,
        ..Default::default()
    };
    assert_eq!(
        registration.evaluate(&Overrides::default()).unwrap(),
        registration.defaults
    );
    for keep in [0, 11] {
        assert_eq!(
            registration
                .evaluate(&Overrides {
                    keep: Some(keep),
                    ..Default::default()
                })
                .unwrap_err()
                .code,
            ErrorCode::PolicyConflict
        );
    }
    assert!(
        registration
            .evaluate(&Overrides {
                keep: Some(5),
                ..Default::default()
            })
            .is_ok()
    );
    registration.defaults.output.protection =
        Some(saucepan::models::policy::Protection::UseableApp);
    assert!(
        registration
            .evaluate(&Overrides {
                useable_app: Some(false),
                ..Default::default()
            })
            .is_err()
    );
    assert!(
        serde_json::from_value::<ReclaimPolicy>(
            json!({"type":"reclaim_policy","strategy":"invented"})
        )
        .is_err()
    );
    assert!(serde_json::from_value::<Overrides>(json!({"secret_override":true})).is_err());
    registration.minimum_keep = 20;
    assert!(registration.validate().is_err());
}

#[test]
fn secrets_and_error_causes_have_safe_debug_and_machine_codes() {
    let credential = Credential::from_bytes(&[123; 32]).unwrap();
    assert_eq!(format!("{credential:?}"), "Credential([REDACTED])");
    assert_eq!(credential.expose(), &[123; 32]);
    assert!(Credential::from_bytes(&[1; 31]).is_err());
    let policy = CredentialPolicy::Required {
        generation: 1,
        verifier: [123; 32],
    };
    assert!(!format!("{policy:?}").contains("123"));
    let e = anyhow::Error::new(Error::new(
        ErrorCode::AccessDenied,
        "custom translated message",
    ))
    .context("outer context");
    assert_eq!(
        serde_json::to_value(Error::from_anyhow(&e)).unwrap()["code"],
        "access_denied"
    );
    let arbitrary = anyhow::anyhow!("secret token must not leak");
    assert!(!format!("{:?}", Error::from_anyhow(&arbitrary)).contains("secret token"));
}

#[test]
fn registry_and_claim_round_trip_validate_the_new_contract() {
    use saucepan::models::{
        claim::{Claim, Registry},
        store::{Request, Selector},
    };
    let mut registry = Registry {
        app: Uuid::new_v4(),
        revision: 0,
        registration_key: scope("integration"),
        access: Default::default(),
        policies: Default::default(),
        credential: Default::default(),
        members: Default::default(),
    };
    registry.validate().unwrap();
    let encoded = serde_json::to_value(&registry).unwrap();
    serde_json::from_value::<Registry>(encoded.clone())
        .unwrap()
        .validate()
        .unwrap();
    let mut invalid = encoded;
    invalid["unknown"] = json!(true);
    assert!(serde_json::from_value::<Registry>(invalid).is_err());
    registry.members.insert(Uuid::new_v4(), "wrong".into());
    assert_eq!(
        registry.validate().unwrap_err().code,
        ErrorCode::IntegrityError
    );
    let mut claim = Claim {
        uuid: Uuid::new_v4(),
        scope: scope("project-a"),
        entry_uuid: Uuid::new_v4(),
        request: Request {
            source_uuid: Uuid::new_v4(),
            selector: Selector::Local,
            selection: Default::default(),
            policies: Default::default(),
        },
        context: Default::default(),
        overrides: Default::default(),
        effective: Default::default(),
    };
    claim.validate().unwrap();
    let request_before = serde_json::to_value(&claim.request).unwrap();
    claim.scope = scope("project-b");
    claim.context.insert("project".into(), "b".into());
    assert_eq!(
        request_before,
        serde_json::to_value(&claim.request).unwrap()
    );
    let decoded: Claim = serde_json::from_value(serde_json::to_value(&claim).unwrap()).unwrap();
    decoded.validate().unwrap();
    assert_eq!(decoded.uuid, claim.uuid);
    claim.context.insert("".into(), "invalid".into());
    assert_eq!(
        claim.validate().unwrap_err().code,
        ErrorCode::InvalidRequest
    );
}
