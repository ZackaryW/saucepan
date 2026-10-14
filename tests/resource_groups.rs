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

