mod native_root;
mod policy_fixture;
use saucepan::{
    core::Store,
    models::{
        group::GroupDefinition,
        source::Source,
        store::{Request, Selector},
    },
};
use serde_json::json;
use std::fs;

fn fixture() -> (tempfile::TempDir, Store, GroupDefinition) {
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    fs::write(tmp.path().join("not-yet-created"), "initial").unwrap();
    let source = store
        .register_source(Source::Local {
            path: tmp.path().join("not-yet-created"),
        })
        .unwrap();
    let definition = serde_json::from_value(json!({"registration_key":"group", "members":[
        {"name":"command","target":{"type":"acquire","source_uuid":source.uuid,"selector":{"type":"local"}}}
    ]})).unwrap();
    (tmp, store, definition)
}

#[test]
fn registration_is_pure_concurrent_idempotent_and_revision_checked() {
    let (_tmp, store, definition) = fixture();
    let barrier = std::sync::Barrier::new(3);
    let groups = std::thread::scope(|s| {
        let threads: Vec<_> = (0..2)
            .map(|_| {
                s.spawn(|| {
                    barrier.wait();
                    store.register_group(&definition).unwrap()
                })
            })
            .collect();
        barrier.wait();
        threads
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(groups[0], groups[1]);
    let group = groups[0].clone();
    assert!(group.selected.is_none());
    assert_eq!(fs::read_dir(store.root().join("store")).unwrap().count(), 0);
    assert_eq!(store.groups(Some("group")).unwrap(), vec![group.clone()]);
    let mut replacement = definition.clone();
    replacement.members[0].name = "different".into();
    assert!(store.register_group(&replacement).is_err());
    assert!(store.replace_group(group.uuid, 0, &replacement).is_err());
    let replaced = store.replace_group(group.uuid, 1, &replacement).unwrap();
    assert_eq!(replaced.revision, 2);
    let reopened = native_root::open(store.root()).unwrap();
    assert_eq!(reopened.group(group.uuid).unwrap(), replaced);
    assert!(reopened.remove_group(group.uuid, 1).is_err());
    reopened.remove_group(group.uuid, 2).unwrap();
    assert!(reopened.groups(None).unwrap().is_empty());
}

#[test]
fn models_and_policy_roles_reject_unusable_persistent_intents() {
    let (_tmp, store, definition) = fixture();
    let mut value = serde_json::to_value(&definition).unwrap();
    value["members"][0]["relative"] = json!("../escape");
    assert!(serde_json::from_value::<GroupDefinition>(value).is_err());
    let mut bad = definition.clone();
    bad.members.push(bad.members[0].clone());
    assert!(store.register_group(&bad).is_err());
    bad = definition.clone();
    bad.policy_refs = policy_fixture::definitions(
        &store,
        vec![json!({"type":"pin_policy","retention":"until_released"})],
    )
    .unwrap();
    assert!(store.register_group(&bad).is_err());
    for policy in [
        json!({"type":"cache_policy","mode":"no_store"}),
        json!({"type":"cache_policy","mode":"temporary"}),
        json!({"type":"pin_policy","retention":"until_released"}),
    ] {
        bad = definition.clone();
        let saucepan::models::group::Target::Acquire(input) = &mut bad.members[0].target else {
            unreachable!()
        };
        input.policy_refs = policy_fixture::definitions(&store, vec![policy]).unwrap();
        assert!(store.register_group(&bad).is_err());
    }
    bad = definition.clone();
    bad.policy_refs = policy_fixture::definitions(
        &store,
        vec![json!({"type":"access_policy","actions":["lease"]})],
    )
    .unwrap();
    assert!(store.register_group(&bad).is_err());
    assert!(store.groups(None).unwrap().is_empty());
    let mut value = serde_json::to_value(&definition).unwrap();
    value["extra"] = json!(true);
    assert!(serde_json::from_value::<GroupDefinition>(value).is_err());
    let mut too_many = definition.clone();
    too_many.members = (0..129)
        .map(|n| {
            let mut member = definition.members[0].clone();
            member.name = format!("member_{n}");
            member
        })
        .collect();
    assert!(store.register_group(&too_many).is_err());
    for block in include_str!("../docs/resource-groups.md")
        .split("```json\n")
        .skip(1)
    {
        serde_json::from_str::<GroupDefinition>(block.split("```").next().unwrap()).unwrap();
    }
}

#[test]
fn group_catalog_is_verified_with_injected_custody() {
    use saucepan::{core::claim::security::MemoryKeys, models::claim::security::IntegrityMode};
    let tmp = tempfile::tempdir().unwrap();
    let keys = MemoryKeys::default();
    let root = tmp.path().join("root");
    let store =
        Store::create_with_integrity_and_keys(&root, IntegrityMode::Keyring, &keys).unwrap();
    let source = store
        .register_source(Source::Url {
            url: "https://example.invalid/file".into(),
        })
        .unwrap();
    let definition = serde_json::from_value(json!({"registration_key":"remote", "members":[
        {"name":"file","target":{"type":"acquire","source_uuid":source.uuid,"selector":{"type":"url"},"selection":{"format":"raw","filename":"payload"}}}
    ]})).unwrap();
    // Registration succeeds without ever contacting this intentionally invalid host.
    let group = store.register_group(&definition).unwrap();
    assert_eq!(
        Store::open_with_keys(&root, &keys)
            .unwrap()
            .group(group.uuid)
            .unwrap(),
        group
    );
    fs::write(root.join(format!("groups/{}/index.json", group.uuid)), "{}").unwrap();
    assert!(store.groups(None).is_err());
    assert!(Store::open_with_keys(&root, &keys).is_err());
}

#[test]
fn app_groups_are_separate_and_cleanup_does_not_require_live_claims() {
    let (tmp, store, mut definition) = fixture();
    let a = store.register_app("a".into()).unwrap();
    let b = store.register_app("b".into()).unwrap();
    let sa = store.session(a.uuid, None).unwrap();
    let sb = store.session(b.uuid, None).unwrap();
    assert!(sa.register_group(&definition).is_err());
    let saucepan::models::group::Target::Acquire(input) = &mut definition.members[0].target else {
        unreachable!()
    };
    input.claim = Some(serde_json::from_value(json!({"scope":"project"})).unwrap());
    assert!(store.register_group(&definition).is_err());
    let ga = sa.register_group(&definition).unwrap();
    let gb = sb.register_group(&definition).unwrap();
    assert_ne!(ga.uuid, gb.uuid);
    assert!(sa.group(gb.uuid).is_err());
    assert_eq!(sa.groups(None).unwrap().len(), 1);
    fs::write(tmp.path().join("not-yet-created"), "data").unwrap();
    let saucepan::models::group::Target::Acquire(input) = &definition.members[0].target else {
        unreachable!()
    };
    let claim = store
        .acquire_for(
            a.uuid,
            &"project".to_owned().try_into().unwrap(),
            &Request {
                source_uuid: input.source_uuid,
                selector: Selector::Local,
                selection: Default::default(),
                policy_refs: Default::default(),
            },
            &Default::default(),
        )
        .unwrap();
    definition.registration_key = "claim-group".to_owned().try_into().unwrap();
    definition.members[0].target = saucepan::models::group::Target::Claim {
        uuid: claim.claim.uuid,
    };
    let cg = sa.register_group(&definition).unwrap();
    sa.release(claim.claim.uuid).unwrap();
    assert_eq!(sa.group(cg.uuid).unwrap(), cg);
    sa.remove_group(cg.uuid, 1).unwrap();
    assert!(store.path(claim.claim.entry_uuid).unwrap().exists());
    sa.uninstall(false, None).unwrap();
    assert!(store.group(ga.uuid).is_err());
    assert!(sb.group(gb.uuid).is_ok());
}

#[test]
fn local_preparation_is_explicit_manual_and_retained_through_removal() {
    let (tmp, store, definition) = fixture();
    let group = store.register_group(&definition).unwrap();
    assert!(store.open_group(group.uuid).is_err());
    let prepared = store
        .prepare_group(group.uuid, Some(1), &Default::default())
        .unwrap();
    assert_eq!(prepared.group.revision, 2);
    let id = prepared.lease.member("command").unwrap().output_uuid;
    let path = prepared.lease.member("command").unwrap().directory.clone();
    fs::write(tmp.path().join("not-yet-created"), "changed").unwrap();
    let repeated = store
        .prepare_group(group.uuid, Some(2), &Default::default())
        .unwrap();
    assert_eq!(repeated.group, prepared.group);
    assert_eq!(
        store
            .open_group(group.uuid)
            .unwrap()
            .lease
            .member("command")
            .unwrap()
            .output_uuid,
        id
    );
    store.remove_group(group.uuid, 2).unwrap();
    assert!(path.exists());
    assert!(store.group(group.uuid).is_err());
}

#[test]
fn fresh_covered_members_converge_before_publication_in_separate_scopes() {
    let (tmp, store, _) = fixture();
    let input = tmp.path().join("tree");
    fs::create_dir_all(input.join("plugins/child")).unwrap();
    fs::write(input.join("plugins/child/file"), "data").unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let app = store.register_app("consumer".into()).unwrap();
    let session = store.session(app.uuid, None).unwrap();
    let definition: GroupDefinition = serde_json::from_value(json!({"registration_key":"covered", "members":[
        {"name":"child","target":{"type":"acquire","source_uuid":source.uuid,"selector":{"type":"local"},"selection":{"folder":"plugins/child"},"claim":{"scope":"project"}}},
        {"name":"parent","target":{"type":"acquire","source_uuid":source.uuid,"selector":{"type":"local"},"selection":{"folder":"plugins"},"claim":{"scope":"project"}}},
        {"name":"other","target":{"type":"acquire","source_uuid":source.uuid,"selector":{"type":"local"},"selection":{"folder":"plugins"},"claim":{"scope":"another"}}}
    ]})).unwrap();
    let group = session.register_group(&definition).unwrap();
    let revision = store.application(app.uuid).unwrap().revision;
    let prepared = session
        .prepare_group(group.uuid, None, &Default::default())
        .unwrap();
    assert_eq!(store.claims(app.uuid).unwrap().len(), 2);
    assert_eq!(store.application(app.uuid).unwrap().revision, revision + 1);
    let parent = prepared.lease.member("parent").unwrap();
    let child = prepared.lease.member("child").unwrap();
    assert_eq!(parent.claim_uuid, child.claim_uuid);
    assert_eq!(child.directory, parent.directory.join("child"));
    assert_ne!(
        parent.claim_uuid,
        prepared.lease.member("other").unwrap().claim_uuid
    );
    assert_eq!(
        session
            .prepare_group(group.uuid, None, &Default::default())
            .unwrap()
            .group,
        prepared.group
    );
    session.release(parent.claim_uuid.unwrap()).unwrap();
    assert!(session.open_group(group.uuid).is_err());
    assert!(parent.directory.exists());
}

#[test]
fn failed_group_update_leaves_all_claims_checks_and_selections_unchanged() {
    let (tmp, store, _) = fixture();
    let input = tmp.path().join("tree");
    fs::create_dir_all(input.join("one")).unwrap();
    fs::create_dir_all(input.join("two")).unwrap();
    fs::write(input.join("one/file"), "a").unwrap();
    fs::write(input.join("two/file"), "a").unwrap();
    let source = store
        .register_source(Source::Local {
            path: input.clone(),
        })
        .unwrap();
    let app = store.register_app("consumer".into()).unwrap();
    let session = store.session(app.uuid, None).unwrap();
    let policies = policy_fixture::definitions(
        &store,
        vec![json!({"type":"update_policy","mode":"always"})],
    )
    .unwrap();
    let definition: GroupDefinition = serde_json::from_value(json!({"registration_key":"pair", "members":[
        {"name":"one","target":{"type":"acquire","source_uuid":source.uuid,"selector":{"type":"local"},"selection":{"folder":"one"},"claim":{"scope":"project"},"policy_refs":policies}},
        {"name":"two","target":{"type":"acquire","source_uuid":source.uuid,"selector":{"type":"local"},"selection":{"folder":"two"},"claim":{"scope":"project"},"policy_refs":policies}}
    ]})).unwrap();
    let group = session.register_group(&definition).unwrap();
    let prepared = session
        .prepare_group(group.uuid, None, &Default::default())
        .unwrap();
    let before = serde_json::to_value(store.claims(app.uuid).unwrap()).unwrap();
    let checks = |store: &Store| {
        store
            .records()
            .unwrap()
            .into_iter()
            .filter_map(|r| match r {
                saucepan::models::index::Record::Acquisition(_) => {
                    Some(serde_json::to_value(r).unwrap())
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let old_checks = checks(&store);
    fs::write(input.join("one/file"), "b").unwrap();
    fs::remove_dir_all(input.join("two")).unwrap();
    assert!(
        session
            .prepare_group(group.uuid, None, &Default::default())
            .is_err()
    );
    assert_eq!(session.group(group.uuid).unwrap(), prepared.group);
    assert_eq!(
        serde_json::to_value(store.claims(app.uuid).unwrap()).unwrap(),
        before
    );
    assert_eq!(checks(&store), old_checks);
}

#[test]
fn prepare_and_pin_receipt_retries_keep_old_generations_after_claim_movement() {
    let (tmp, store, mut definition) = fixture();
    let app = store.register_app("plugin-host".into()).unwrap();
    let session = store.session(app.uuid, None).unwrap();
    let saucepan::models::group::Target::Acquire(input) = &mut definition.members[0].target else {
        unreachable!()
    };
    input.claim = Some(serde_json::from_value(json!({"scope":"plugins"})).unwrap());
    input.policy_refs = policy_fixture::definitions(
        &store,
        vec![
            json!({"type":"update_policy","mode":"always"}),
            json!({"type":"revision_policy","history":"none"}),
        ],
    )
    .unwrap();
    let group = session.register_group(&definition).unwrap();
    let mut pin = saucepan::models::pin::GroupPinRequest {
        registration_key: "session/a".to_owned().try_into().unwrap(),
        policy_refs: policy_fixture::definitions(
            &store,
            vec![json!({"type":"pin_policy","retention":"until_released"})],
        )
        .unwrap(),
    };
    let a = session
        .prepare_group_with_pin(group.uuid, 1, &Default::default(), &pin)
        .unwrap();
    let aid = a.lease.member("command").unwrap().output_uuid;
    let a_pin = a.pin.clone().unwrap();
    let a_group = a.group.clone();
    drop(a);
    fs::write(tmp.path().join("not-yet-created"), "next").unwrap();
    let retry = session
        .prepare_group_with_pin(group.uuid, 1, &Default::default(), &pin)
        .unwrap();
    assert_eq!(retry.group, a_group);
    assert_eq!(retry.pin.as_ref().unwrap().uuid, a_pin.uuid);
    drop(retry);
    pin.registration_key = "session/b".to_owned().try_into().unwrap();
    let b = session
        .prepare_group_with_pin(group.uuid, a_group.revision, &Default::default(), &pin)
        .unwrap();
    assert_ne!(b.lease.member("command").unwrap().output_uuid, aid);
    assert!(store.reclaim().unwrap().deferred.contains_key(&aid));
    pin.registration_key = a_pin.request.registration_key.clone();
    let retry = session
        .prepare_group_with_pin(group.uuid, 1, &Default::default(), &pin)
        .unwrap();
    assert_eq!(retry.lease.member("command").unwrap().output_uuid, aid);
    drop(retry);
    session.unpin(a_pin.uuid).unwrap();
    assert!(
        session
            .prepare_group_with_pin(group.uuid, 1, &Default::default(), &pin)
            .is_err()
    );
    assert!(store.reclaim().unwrap().reclaimed.contains(&aid));
    let standalone = saucepan::models::pin::GroupPinRequest {
        registration_key: "extra".to_owned().try_into().unwrap(),
        policy_refs: pin.policy_refs.clone(),
    };
    let retained = session
        .pin_group(group.uuid, b.group.revision, &standalone)
        .unwrap();
    assert_eq!(
        session
            .pin_group(group.uuid, b.group.revision, &standalone)
            .unwrap(),
        retained
    );
    assert!(session.pin_group(group.uuid, 1, &standalone).is_err());
}

#[test]
fn compatible_git_members_select_one_commit_while_manual_members_keep_their_revision() {
    fn git(path: &std::path::Path, args: &[&str]) -> String {
        let result = std::process::Command::new("git")
            .arg("-C")
            .arg(path)
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8_lossy(&result.stdout).trim().into()
    }
    let (tmp, store, _) = fixture();
    let origin = tmp.path().join("origin");
    fs::create_dir(&origin).unwrap();
    git(&origin, &["init", "-b", "main"]);
    for name in ["command", "hooks", "manual"] {
        fs::create_dir(origin.join(name)).unwrap();
        fs::write(origin.join(name).join("file"), "one").unwrap();
    }
    git(&origin, &["add", "."]);
    git(&origin, &["commit", "-m", "one"]);
    let first_commit = git(&origin, &["rev-parse", "HEAD"]);
    let source = store
        .register_source(Source::Git {
            origin: origin.to_str().unwrap().into(),
        })
        .unwrap();
    let refs = policy_fixture::definitions(
        &store,
        vec![
            json!({"type":"update_policy","mode":"always"}),
            json!({"type":"stale_policy","allow_stale":true}),
        ],
    )
    .unwrap();
    let definition = serde_json::from_value(json!({"registration_key":"zush", "members":[
        {"name":"command","target":{"type":"acquire","source_uuid":source.uuid,"selector":{"type":"ref","reference":"main"},"selection":{"folder":"command"},"policy_refs":refs}},
        {"name":"hooks","target":{"type":"acquire","source_uuid":source.uuid,"selector":{"type":"ref","reference":"main"},"selection":{"folder":"hooks"},"policy_refs":refs}},
        {"name":"manual","target":{"type":"acquire","source_uuid":source.uuid,"selector":{"type":"ref","reference":"main"},"selection":{"folder":"manual"}}}
    ]})).unwrap();
    let group = store.register_group(&definition).unwrap();
    let first = store
        .prepare_group(group.uuid, None, &Default::default())
        .unwrap();
    for name in ["command", "hooks", "manual"] {
        fs::write(origin.join(name).join("file"), "two").unwrap();
    }
    git(&origin, &["add", "."]);
    git(&origin, &["commit", "-m", "two"]);
    let next_commit = git(&origin, &["rev-parse", "HEAD"]);
    let second = store
        .prepare_group(group.uuid, None, &Default::default())
        .unwrap();
    for name in ["command", "hooks"] {
        let member = second.lease.member(name).unwrap();
        assert_eq!(
            store.output(member.output_uuid).unwrap().resolved,
            saucepan::models::source::Resolved::Git {
                commit: next_commit.clone()
            }
        );
        assert_eq!(fs::read(member.directory.join("file")).unwrap(), b"two");
    }
    let member = second.lease.member("manual").unwrap();
    assert_eq!(
        member.output_uuid,
        first.lease.member("manual").unwrap().output_uuid
    );
    assert_eq!(
        store.output(member.output_uuid).unwrap().resolved,
        saucepan::models::source::Resolved::Git {
            commit: first_commit
        }
    );
    fs::rename(&origin, tmp.path().join("offline-origin")).unwrap();
    let stale = store
        .prepare_group(group.uuid, None, &Default::default())
        .unwrap();
    assert_eq!(stale.group, second.group);
    assert_eq!(
        stale.stale,
        ["command".to_owned(), "hooks".to_owned()].into()
    );
}
