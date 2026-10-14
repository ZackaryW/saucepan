use super::*;
use crate::{
    models::{pin::GroupPinRequest, source::Source, store::Availability},
    utils::lock::{LockGuard, LockMode},
};
use serde_json::json;
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn fixture(
    reclaim: &str,
) -> (
    tempfile::TempDir,
    Store,
    AppSession,
    GroupRecord,
    GroupPinRequest,
) {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
    let input = tmp.path().join("tree");
    fs::create_dir_all(input.join("one")).unwrap();
    fs::create_dir_all(input.join("two")).unwrap();
    fs::write(input.join("one/file"), "a").unwrap();
    fs::write(input.join("two/file"), "a").unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let app = store.register_app("app".into()).unwrap();
    let session = store.session(app.uuid, None).unwrap();
    let policies = crate::policy_fixture::definitions(
        &store,
        vec![
            json!({"type":"update_policy","mode":"always"}),
            json!({"type":"reclaim_policy","strategy":reclaim}),
            json!({"type":"revision_policy","history":"none"}),
        ],
    )
    .unwrap();
    let definition = serde_json::from_value(json!({"registration_key":"pair", "members":[
        {"name":"one","target":{"type":"acquire","source_uuid":source.uuid,"selector":{"type":"local"},"selection":{"folder":"one"},"claim":{"scope":"project"},"policy_refs":policies}},
        {"name":"two","target":{"type":"acquire","source_uuid":source.uuid,"selector":{"type":"local"},"selection":{"folder":"two"},"claim":{"scope":"project"},"policy_refs":policies}}
    ]})).unwrap();
    let group = session.register_group(&definition).unwrap();
    let pin = GroupPinRequest {
        registration_key: "key".to_owned().try_into().unwrap(),
        policy_refs: crate::policy_fixture::definitions(
            &store,
            vec![json!({"type":"pin_policy","retention":"until_released"})],
        )
        .unwrap(),
    };
    (tmp, store, session, group, pin)
}

#[test]
fn publication_recovery_keeps_claims_checks_group_and_pin_in_one_transaction() {
    let mut events = 0;
    for fault in 0..12 {
        if fault > 0 && fault > events {
            break;
        }
        let (_tmp, store, mut session, group, pin) = fixture("report_unavailable");
        let count = Arc::new(AtomicUsize::new(0));
        let observed = count.clone();
        session.store.group_publication_observer = Some(Arc::new(move |_| {
            if observed.fetch_add(1, Ordering::SeqCst) + 1 == fault {
                return Err(Error::new(
                    ErrorCode::SourceError,
                    "injected group journal interruption",
                ));
            }
            Ok(())
        }));
        let result = session.prepare_group_with_pin(group.uuid, 1, &Default::default(), &pin);
        if fault == 0 {
            events = count.load(Ordering::SeqCst);
            assert!(result.is_ok());
        } else {
            assert!(result.is_err());
        }
        drop(result);
        let reopened = Store::open_fixture(store.root()).unwrap();
        let recovered = reopened.session(session.app(), None).unwrap();
        let selected = recovered.group(group.uuid).unwrap();
        assert_eq!(selected.revision, 2);
        assert_eq!(recovered.pins(None).unwrap().len(), 1);
        assert_eq!(reopened.claims(session.app()).unwrap().len(), 2);
        let checks: Vec<_> = reopened
            .records()
            .unwrap()
            .into_iter()
            .filter_map(|r| match r {
                Record::Acquisition(s) => Some(s),
                _ => None,
            })
            .collect();
        assert_eq!(checks.len(), 2);
        for member in selected.selected.as_ref().unwrap().members.values() {
            assert!(checks.iter().any(|c| c.selected == member.output_uuid));
            assert_eq!(
                reopened.registry(session.app()).unwrap().claims[&member.claim_uuid.unwrap()]
                    .entry_uuid,
                member.output_uuid
            );
        }
        // Recovery returns the receipt rather than executing another always update.
        assert_eq!(
            recovered
                .prepare_group_with_pin(group.uuid, 1, &Default::default(), &pin)
                .unwrap()
                .group,
            selected
        );
    }
    assert!(events >= 8);
}

#[test]
fn final_fences_reject_group_claim_and_credential_edits_without_resurrection() {
    for change in [
        "release",
        "update",
        "adoption",
        "source_policy",
        "context",
        "replace",
        "remove",
        "credentials",
        "uninstall",
    ] {
        let (tmp, store, mut session, group, _) = fixture("report_unavailable");
        let previous = session
            .prepare_group(group.uuid, None, &Default::default())
            .unwrap()
            .group;
        fs::write(tmp.path().join("tree/one/file"), "b").unwrap();
        let editor = store.clone();
        let app = session.app();
        let gid = group.uuid;
        let claim = previous.selected.as_ref().unwrap().members["one"]
            .claim_uuid
            .unwrap();
        let replacement = group.definition.clone();
        session.store.before_claim_commit = Some(Arc::new(move || match change {
            "update" => {
                editor
                    .update_claim(app, claim, &Default::default())
                    .unwrap();
            }
            "adoption" => {
                let refs = crate::policy_fixture::definitions(
                    &editor,
                    vec![json!({"type":"revision_policy","history":"latest","keep":2})],
                )
                .unwrap();
                editor
                    .adopt_claim_policies(
                        app,
                        claim,
                        editor.application(app).unwrap().revision,
                        refs,
                    )
                    .unwrap();
            }
            "source_policy" => {
                let source = editor
                    .source(
                        editor.registry(app).unwrap().claims[&claim]
                            .request
                            .source_uuid,
                    )
                    .unwrap();
                let refs = crate::policy_fixture::definitions(
                    &editor,
                    vec![
                        json!({"type":"verification_policy","policy_target":"path","mode":"full"}),
                    ],
                )
                .unwrap();
                editor
                    .adopt_source_policies(source.uuid, source.policy_revision, refs)
                    .unwrap();
            }
            "release" => editor.release_claim(app, claim).unwrap(),
            "context" => {
                editor
                    .set_claim_context(
                        app,
                        claim,
                        &[("edit".into(), "value".into())].into(),
                        editor.application(app).unwrap().revision,
                    )
                    .unwrap();
            }
            "replace" => {
                let mut d = replacement.clone();
                d.members[0].name = "renamed".into();
                editor.replace_group(gid, 2, &d).unwrap();
            }
            "remove" => editor.remove_group(gid, 2).unwrap(),
            "credentials" => {
                editor
                    .configure_application(
                        app,
                        crate::core::claim::session::ApplicationConfiguration {
                            expected_revision: editor.application(app).unwrap().revision,
                            policy_refs: Default::default(),
                            credential: Some(
                                crate::core::claim::registry::CredentialChange::Enable,
                            ),
                        },
                        None,
                    )
                    .unwrap();
            }
            "uninstall" => {
                editor.uninstall(app, false, None).unwrap();
            }
            _ => unreachable!(),
        }));
        assert!(
            session
                .prepare_group(group.uuid, None, &Default::default())
                .is_err(),
            "{change}"
        );
        if matches!(change, "remove" | "uninstall") {
            assert!(store.group(group.uuid).is_err());
        } else if change != "replace" {
            assert_eq!(store.group(group.uuid).unwrap(), previous);
        }
        for record in store.records().unwrap() {
            if let Record::Output(output) = record {
                assert!(
                    LockGuard::acquire(
                        &store.root().join(format!(".locks/use-{}", output.uuid)),
                        LockMode::Exclusive,
                        Duration::ZERO
                    )
                    .is_ok()
                );
            }
        }
    }
}

#[test]
fn group_claim_recovery_obeys_each_saved_strategy() {
    for strategy in ["report_unavailable", "restore_exact", "refresh"] {
        let (_tmp, store, session, group, _) = fixture(strategy);
        let previous = session
            .prepare_group(group.uuid, None, &Default::default())
            .unwrap()
            .group;
        let old = previous.selected.as_ref().unwrap().members["one"].output_uuid;
        fs::remove_dir_all(store.root().join(format!("store/{old}"))).unwrap();
        let mut record = store.output(old).unwrap();
        record.state = Availability::Reclaimed;
        let _lock = store.lock().unwrap();
        store
            .commit(Journal {
                records: vec![Record::Output(Box::new(record))],
                ..Default::default()
            })
            .unwrap();
        drop(_lock);
        let result = session.prepare_group(group.uuid, None, &Default::default());
        if strategy == "report_unavailable" {
            assert!(result.is_err());
            assert_eq!(session.group(group.uuid).unwrap(), previous);
        } else {
            let next = result.unwrap();
            assert_ne!(next.lease.member("one").unwrap().output_uuid, old);
        }
    }
}

#[test]
fn exhausted_shared_guard_budget_releases_earlier_guards() {
    let (_tmp, store, session, group, _) = fixture("report_unavailable");
    let prepared = session
        .prepare_group(group.uuid, None, &Default::default())
        .unwrap();
    let selected = prepared.group.clone();
    drop(prepared);
    let mut ids: Vec<_> = selected
        .selected
        .as_ref()
        .unwrap()
        .members
        .values()
        .map(|m| m.output_uuid)
        .collect();
    ids.sort();
    let blocked = LockGuard::acquire(
        &store.root().join(format!(".locks/use-{}", ids[1])),
        LockMode::Exclusive,
        Duration::ZERO,
    )
    .unwrap();
    let _lock = store.lock().unwrap();
    assert!(
        session
            .store
            .plan_group(
                selected,
                &Default::default(),
                std::time::Instant::now() - Duration::from_secs(31)
            )
            .is_err()
    );
    assert!(
        LockGuard::acquire(
            &store.root().join(format!(".locks/use-{}", ids[0])),
            LockMode::Exclusive,
            Duration::ZERO
        )
        .is_ok()
    );
    drop(blocked);
}
