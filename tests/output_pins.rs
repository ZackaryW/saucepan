mod native_root;
mod policy_fixture;
use saucepan::{
    core::Store,
    models::{
        pin::{PinRequest, PinState},
        source::Source,
        store::{Request, Selector},
    },
};
use serde_json::json;
use std::{fs, process::Command};
use uuid::Uuid;

struct Fixture {
    tmp: tempfile::TempDir,
    store: Store,
    request: Request,
    pin: PinRequest,
}
impl Fixture {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let input = tmp.path().join("input");
        fs::write(&input, "original").unwrap();
        let store = native_root::create(&tmp.path().join("root")).unwrap();
        let source = store
            .register_source(Source::Local { path: input })
            .unwrap();
        let request = Request {
            source_uuid: source.uuid,
            selector: Selector::Local,
            selection: Default::default(),
            policy_refs: policy_fixture::definitions(
                &store,
                vec![
                    json!({"type":"revision_policy","history":"none"}),
                    json!({"type":"identical_pull_policy","strategy":"new_entry"}),
                ],
            )
            .unwrap(),
        };
        let output = store.acquire(&request).unwrap().entry.uuid;
        let pin = PinRequest {
            registration_key: "consumer/a".to_owned().try_into().unwrap(),
            outputs: vec![output],
            policy_refs: policy_fixture::definitions(
                &store,
                vec![json!({"type":"pin_policy","retention":"until_released"})],
            )
            .unwrap(),
        };
        Self {
            tmp,
            store,
            request,
            pin,
        }
    }
    fn cli(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_saucepan"))
            .arg("--root")
            .arg(self.store.root())
            .args(args)
            .output()
            .unwrap()
    }
}

#[test]
fn updater_exits_independent_pins_survive_until_each_release() {
    let f = Fixture::new();
    let file = f.tmp.path().join("pin.json");
    fs::write(&file, serde_json::to_vec(&f.pin).unwrap()).unwrap();
    let child = f.cli(&["pin-outputs", file.to_str().unwrap()]);
    assert!(
        child.status.success(),
        "{}",
        String::from_utf8_lossy(&child.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&child.stdout).unwrap();
    let first: Uuid = serde_json::from_value(value["pin"]["uuid"].clone()).unwrap();
    let mut second = f.pin.clone();
    second.registration_key = "consumer/b".to_owned().try_into().unwrap();
    let other = f.store.pin_outputs(&second).unwrap();
    assert_eq!(f.store.pin_outputs(&f.pin).unwrap().pin.uuid, first);
    f.store.acquire(&f.request).unwrap();
    let old = f.pin.outputs[0];
    let reopened = native_root::open(f.store.root()).unwrap();
    assert!(reopened.reclaim().unwrap().deferred.contains_key(&old));
    assert_eq!(fs::read_dir(&other.paths[&old]).unwrap().count(), 1);
    assert!(f.cli(&["unpin", &first.to_string()]).status.success());
    assert_eq!(reopened.unpin(first).unwrap().state, PinState::Released);
    assert!(reopened.pin_outputs(&f.pin).is_err());
    assert!(reopened.reclaim().unwrap().deferred.contains_key(&old));
    reopened.unpin(other.pin.uuid).unwrap();
    assert!(reopened.reclaim().unwrap().reclaimed.contains(&old));
    assert!(reopened.unpin(Uuid::new_v4()).is_err());
    assert!(f.cli(&["pins", "--key", "consumer/a"]).status.success());
}

#[test]
fn invalid_roles_targets_and_retries_have_no_partial_effects() {
    let f = Fixture::new();
    for outputs in [
        vec![],
        vec![f.pin.outputs[0]; 2],
        vec![f.pin.outputs[0], Uuid::new_v4()],
    ] {
        let mut request = f.pin.clone();
        request.outputs = outputs;
        assert!(f.store.pin_outputs(&request).is_err());
        assert!(f.store.pins(None).unwrap().is_empty());
    }
    let mut wrong = f.pin.clone();
    wrong.policy_refs = f.request.policy_refs.clone();
    assert!(f.store.pin_outputs(&wrong).is_err());
    wrong.policy_refs = Default::default();
    assert!(f.store.pin_outputs(&wrong).is_err());
    let mut acquire = f.request.clone();
    acquire.policy_refs = f.pin.policy_refs.clone();
    assert!(f.store.acquire(&acquire).is_err());
    let source = f.store.source(f.request.source_uuid).unwrap();
    assert!(
        f.store
            .adopt_source_policies(
                source.uuid,
                source.policy_revision,
                f.pin.policy_refs.clone()
            )
            .is_err()
    );
    assert!(
        serde_json::from_value::<PinRequest>(
            json!({"registration_key":"x","outputs":[],"policy_refs":[],"extra":true})
        )
        .is_err()
    );
    let first = f.store.pin_outputs(&f.pin).unwrap();
    let new = f.store.acquire(&f.request).unwrap();
    wrong = f.pin.clone();
    wrong.outputs = vec![new.entry.uuid];
    assert!(f.store.pin_outputs(&wrong).is_err());
    assert_eq!(f.store.pin(first.pin.uuid).unwrap().request, f.pin);
}

#[test]
fn app_witness_survives_claim_release_and_cleanup_has_its_own_permission() {
    let f = Fixture::new();
    let app = f.store.register_app("consumer".into()).unwrap();
    let claim = f
        .store
        .acquire_for(
            app.uuid,
            &"project".to_owned().try_into().unwrap(),
            &f.request,
            &Default::default(),
        )
        .unwrap();
    let session = f.store.session(app.uuid, None).unwrap();
    let mut request = f.pin.clone();
    request.outputs = vec![claim.claim.entry_uuid];
    let pinned = session.pin_outputs(&request).unwrap();
    session.release(claim.claim.uuid).unwrap();
    assert_eq!(session.pin(pinned.pin.uuid).unwrap(), pinned.pin);
    let other = f.store.register_app("other".into()).unwrap();
    assert!(
        f.store
            .session(other.uuid, None)
            .unwrap()
            .pin(pinned.pin.uuid)
            .is_err()
    );
    let policies = policy_fixture::definitions(
        &f.store,
        vec![json!({"type":"access_policy","actions":["read","unpin"]})],
    )
    .unwrap();
    f.store
        .configure_application(
            app.uuid,
            saucepan::core::claim::session::ApplicationConfiguration {
                expected_revision: f.store.application(app.uuid).unwrap().revision,
                policy_refs: policies,
                credential: None,
            },
            None,
        )
        .unwrap();
    assert!(session.pin_outputs(&request).is_err());
    // Cleanup does not touch or require payload data.
    fs::remove_dir_all(&pinned.paths[&claim.claim.entry_uuid]).unwrap();
    assert_eq!(
        session.unpin(pinned.pin.uuid).unwrap().state,
        PinState::Released
    );
}

#[test]
fn corruption_prevents_any_reclaim_deletion() {
    let f = Fixture::new();
    let pin = f.store.pin_outputs(&f.pin).unwrap();
    f.store.acquire(&f.request).unwrap();
    fs::write(
        f.store.root().join(format!("pins/{}.json", pin.pin.uuid)),
        b"{}",
    )
    .unwrap();
    assert!(f.store.reclaim().is_err());
    assert!(pin.paths[&f.pin.outputs[0]].exists());
}

#[test]
fn concurrent_registration_converges_and_reclaim_cannot_delete_an_admitted_pin() {
    let f = Fixture::new();
    let barrier = std::sync::Barrier::new(3);
    std::thread::scope(|s| {
        let threads: Vec<_> = (0..2)
            .map(|_| {
                s.spawn(|| {
                    barrier.wait();
                    f.store.pin_outputs(&f.pin).unwrap().pin.uuid
                })
            })
            .collect();
        barrier.wait();
        let ids: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        assert_eq!(ids[0], ids[1]);
    });
    f.store.acquire(&f.request).unwrap();
    assert!(
        f.store
            .reclaim()
            .unwrap()
            .deferred
            .contains_key(&f.pin.outputs[0])
    );
}

#[test]
fn documentation_payloads_parse_and_cli_is_bounded_without_identity_fallback() {
    let examples: Vec<_> = include_str!("../docs/output-pins.md")
        .split("```json\n")
        .skip(1)
        .map(|s| s.split("```").next().unwrap())
        .collect();
    assert_eq!(examples.len(), 2);
    serde_json::from_str::<Vec<saucepan::models::policy::PolicyRecord>>(examples[0]).unwrap();
    serde_json::from_str::<PinRequest>(examples[1])
        .unwrap()
        .validate()
        .unwrap();
    let f = Fixture::new();
    let file = f.tmp.path().join("oversize.json");
    fs::write(&file, vec![b' '; 1024 * 1024 + 1]).unwrap();
    let out = f.cli(&["pin-outputs", file.to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(
        !f.cli(&["--credential-file", "missing", "pins"])
            .status
            .success()
    );
    assert!(
        !f.cli(&["--as-app", &Uuid::new_v4().to_string(), "pins"])
            .status
            .success()
    );
}

#[test]
fn full_verification_failure_rolls_back_all_guards_and_pins() {
    let f = Fixture::new();
    let keep = f.store.execution_lease(f.pin.outputs[0]).unwrap();
    let second = f.store.acquire(&f.request).unwrap().entry.uuid;
    let source = f.store.source(f.request.source_uuid).unwrap();
    let refs = policy_fixture::definitions(
        &f.store,
        vec![json!({"type":"verification_policy","policy_target":"path","mode":"full"})],
    )
    .unwrap();
    f.store
        .adopt_source_policies(source.uuid, source.policy_revision, refs)
        .unwrap();
    let path = f.store.path(second).unwrap();
    let file = fs::read_dir(path).unwrap().next().unwrap().unwrap().path();
    fs::write(file, "tampered").unwrap();
    let mut request = f.pin.clone();
    request.outputs.push(second);
    assert!(f.store.pin_outputs(&request).is_err());
    assert!(f.store.pins(None).unwrap().is_empty());
    drop(keep);
    assert!(
        f.store
            .reclaim()
            .unwrap()
            .reclaimed
            .contains(&f.pin.outputs[0])
    );
    assert!(f.store.pin_outputs(&f.pin).is_err());
}

#[test]
fn protected_store_pin_survives_uninstall_and_is_administratively_releasable() {
    use saucepan::{
        core::claim::{security::MemoryKeys, session::ApplicationRegistration},
        models::claim::security::IntegrityMode,
    };
    let tmp = tempfile::tempdir().unwrap();
    let keys = MemoryKeys::default();
    let root = tmp.path().join("root");
    let store =
        Store::create_with_integrity_and_keys(&root, IntegrityMode::Keyring, &keys).unwrap();
    let input = tmp.path().join("input");
    fs::write(&input, "payload").unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let registered = store
        .register_application(
            &ApplicationRegistration {
                registration_key: "app".to_owned().try_into().unwrap(),
                name: "app".into(),
                credentials_required: true,
                policy_refs: Default::default(),
            },
            None,
        )
        .unwrap();
    let app = registered.application.uuid;
    assert!(store.session(app, None).is_err());
    let session = store.session(app, registered.credential).unwrap();
    let claim = session
        .ensure(
            &serde_json::from_value(
                json!({"scope":"project","source_uuid":source.uuid,"selector":{"type":"local"}}),
            )
            .unwrap(),
            &Default::default(),
        )
        .unwrap();
    let pin = session
        .pin_outputs(&PinRequest {
            registration_key: "process".to_owned().try_into().unwrap(),
            outputs: vec![claim.claim.entry_uuid],
            policy_refs: policy_fixture::definitions(
                &store,
                vec![json!({"type":"pin_policy","retention":"until_released"})],
            )
            .unwrap(),
        })
        .unwrap();
    session.uninstall(false, None).unwrap();
    assert!(session.pin(pin.pin.uuid).is_err());
    let reopened = Store::open_with_keys(&root, &keys).unwrap();
    assert_eq!(reopened.pin(pin.pin.uuid).unwrap().state, PinState::Active);
    assert!(pin.paths[&claim.claim.entry_uuid].exists());
    reopened.unpin(pin.pin.uuid).unwrap();
}
