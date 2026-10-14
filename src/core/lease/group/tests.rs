use super::*;
use crate::{
    core::claim::session::{ApplicationConfiguration, ApplicationRegistration},
    models::{
        claim::EnsureRequest,
        policy::{PolicyDefinition, PolicyRecord, record::VerificationPolicy},
        source::Source,
        store::{Request, Selector},
    },
    utils::{
        lock::{LockGuard, LockMode},
        tree::IoEvent,
    },
};
use serde_json::json;
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

fn fixture() -> (tempfile::TempDir, Store, Request, Uuid) {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("input");
    fs::create_dir_all(input.join("hooks")).unwrap();
    fs::write(input.join("hooks/script"), "payload").unwrap();
    let store = Store::create_fixture(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let mut request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policy_refs: Default::default(),
    };
    crate::policy_fixture::replace(
        &store,
        &mut request.policy_refs,
        json!({"type":"revision_policy","history":"latest","keep":1}),
    )
    .unwrap();
    let output = store.acquire(&request).unwrap().entry.uuid;
    (tmp, store, request, output)
}
fn group(id: Uuid) -> GroupRequest {
    serde_json::from_value(json!({"members":[{"name":"root","target":{"type":"output","uuid":id}},{"name":"hooks","target":{"type":"output","uuid":id},"relative":"hooks"}]})).unwrap()
}
fn policy(value: serde_json::Value) -> PolicyRecord {
    let mut value = value;
    value["uuid"] = json!(Uuid::new_v4());
    serde_json::from_value(value).unwrap()
}
fn full() -> PolicyRecord {
    policy(json!({"type":"verification_policy","policy_target":"path","mode":"full"}))
}
fn available(store: &Store, id: Uuid) -> bool {
    LockGuard::acquire(
        &store.root.join(format!(".locks/use-{id}")),
        LockMode::Exclusive,
        Duration::ZERO,
    )
    .is_ok()
}
fn app(store: &Store, request: &Request) -> (AppSession, Uuid) {
    let id = store
        .register_application(
            &ApplicationRegistration {
                registration_key: "test".to_string().try_into().unwrap(),
                name: "consumer".into(),
                credentials_required: false,
                policy_refs: Default::default(),
            },
            None,
        )
        .unwrap()
        .application
        .uuid;
    let session = store.session(id, None).unwrap();
    let claim = session
        .ensure(
            &EnsureRequest {
                scope: "project-a".to_string().try_into().unwrap(),
                source_uuid: request.source_uuid,
                selector: request.selector.clone(),
                selection: request.selection.clone(),
                policy_refs: request.policy_refs.clone(),
                context: Default::default(),
            },
            &Default::default(),
        )
        .unwrap()
        .claim
        .uuid;
    (session, claim)
}
fn claim_group(id: Uuid) -> GroupRequest {
    serde_json::from_value(
        json!({"members":[{"name":"command","target":{"type":"claim","uuid":id}}]}),
    )
    .unwrap()
}

#[test]
fn groups_deduplicate_views_pin_independently_and_do_no_payload_io_by_default() {
    let (tmp, mut store, request, id) = fixture();
    let opens = Arc::new(AtomicUsize::new(0));
    let seen = opens.clone();
    store.access_observer = Some(Arc::new(move |event| {
        if matches!(event, IoEvent::Open) {
            seen.fetch_add(1, Ordering::SeqCst);
        }
    }));
    let before = store.metadata.snapshot().unwrap();
    let a = store.lease_group(&group(id), &Default::default()).unwrap();
    let b = store.lease_group(&group(id), &Default::default()).unwrap();
    assert_eq!(a.output_count(), 1);
    assert_eq!(
        a.member("hooks").unwrap().directory,
        a.member("root").unwrap().directory.join("hooks")
    );
    assert_eq!(opens.load(Ordering::SeqCst), 0);
    assert!(!available(&store, id));
    assert!(!fs::read_dir(store.root.join(".locks")).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("live-")
    }));
    assert_eq!(before.digest, store.metadata.snapshot().unwrap().digest);
    fs::write(tmp.path().join("input/hooks/script"), "new").unwrap();
    store.acquire(&request).unwrap();
    assert!(store.reclaim().unwrap().deferred.contains_key(&id));
    drop(a);
    assert!(!available(&store, id));
    assert_eq!(
        fs::read(b.member("hooks").unwrap().directory.join("script")).unwrap(),
        b"payload"
    );
    drop(b);
    assert!(available(&store, id));
    assert!(store.reclaim().unwrap().reclaimed.contains(&id));
}

#[test]
fn admission_errors_drop_all_guards_and_full_scans_are_deduplicated() {
    let (tmp, mut store, _, id) = fixture();
    let mut request = group(id);
    request.members[1].relative = Some("missing".into());
    assert!(store.lease_group(&request, &Default::default()).is_err());
    assert!(available(&store, id));
    request = group(id);
    let leaf = full();
    request.policy_refs = vec![leaf.uuid].try_into().unwrap();
    let cat = PolicyCatalog::new([leaf]).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    store.access_observer = Some(Arc::new(move |event| {
        if matches!(event, IoEvent::Open) {
            seen.fetch_add(1, Ordering::SeqCst);
        }
    }));
    // Removing upstream input also proves admission never retrieves or recovers it.
    fs::remove_dir_all(tmp.path().join("input")).unwrap();
    let lease = store.lease_group(&request, &cat).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(
        lease
            .descriptor()
            .members
            .values()
            .all(|m| m.verification == VerificationMode::Full)
    );
    let dir = lease.member("root").unwrap().directory.clone();
    drop(lease);
    fs::write(dir.join("hooks/script"), "tampered").unwrap();
    assert!(store.lease_group(&request, &cat).is_err());
    assert!(available(&store, id));
    request.members.clear();
    assert!(store.lease_group(&request, &cat).is_err());
}

#[test]
fn every_member_keeps_its_own_authority_and_scope_witness() {
    let (_tmp, store, request, id) = fixture();
    let (session, claim) = app(&store, &request);
    let mut request = claim_group(claim);
    assert!(store.lease_group(&request, &Default::default()).is_err());
    let scoped = policy(json!({"type":"access_policy","scopes":["project-a"],"actions":["lease"]}));
    request.policy_refs = vec![scoped.uuid].try_into().unwrap();
    let catalog = PolicyCatalog::new([scoped]).unwrap();
    let pinned = session.lease_group(&request, &catalog).unwrap();
    request.members.push(group(id).members.remove(0));
    assert!(session.lease_group(&request, &catalog).is_err()); // same output, no borrowed scope
    assert!(store.lease_group(&request, &catalog).is_err());
    let denied = policy(json!({"type":"access_policy","actions":[]}));
    let mut local = group(id);
    local.policy_refs = vec![denied.uuid].try_into().unwrap();
    assert!(
        store
            .lease_group(&local, &PolicyCatalog::new([denied]).unwrap())
            .is_err()
    );
    let deny = policy(json!({"type":"access_policy","actions":["read"]}));
    store
        .register_policies(std::slice::from_ref(&deny))
        .unwrap();
    store
        .adopt_application_policies(
            session.app(),
            store.application(session.app()).unwrap().revision,
            vec![deny.uuid].try_into().unwrap(),
            None,
        )
        .unwrap();
    assert!(
        session
            .lease_group(&claim_group(claim), &Default::default())
            .is_err()
    );
    assert_eq!(
        fs::read(
            pinned
                .member("command")
                .unwrap()
                .directory
                .join("hooks/script")
        )
        .unwrap(),
        b"payload"
    );
}

#[test]
fn group_metadata_cannot_override_registered_full_verification() {
    let (_tmp, mut store, request, id) = fixture();
    let leaf = full();
    store
        .register_policies(std::slice::from_ref(&leaf))
        .unwrap();
    store
        .adopt_source_policies(request.source_uuid, 0, vec![leaf.uuid].try_into().unwrap())
        .unwrap();
    let metadata =
        policy(json!({"type":"verification_policy","policy_target":"path","mode":"metadata"}));
    let mut request = group(id);
    request.policy_refs = vec![metadata.uuid].try_into().unwrap();
    let seen = Arc::new(AtomicUsize::new(0));
    let observed = seen.clone();
    store.access_observer = Some(Arc::new(move |event| {
        if matches!(event, IoEvent::Open) {
            observed.fetch_add(1, Ordering::SeqCst);
        }
    }));
    let lease = store
        .lease_group(&request, &PolicyCatalog::new([metadata]).unwrap())
        .unwrap();
    assert_eq!(seen.load(Ordering::SeqCst), 1);
    assert_eq!(
        lease.member("root").unwrap().verification,
        VerificationMode::Full
    );
}

#[test]
fn relevant_changes_during_scan_fail_but_unrelated_catalog_writes_do_not() {
    for change in [
        "update",
        "release",
        "uninstall",
        "credentials",
        "source-policy",
        "app-policy",
        "unrelated",
    ] {
        let (tmp, store, request, id) = fixture();
        let (mut session, claim) = app(&store, &request);
        let mut request = claim_group(claim);
        let leaf = full();
        request.policy_refs = vec![leaf.uuid].try_into().unwrap();
        let catalog = PolicyCatalog::new([leaf]).unwrap();
        let editor = store.clone();
        let writer = session.clone();
        let source = store.output(id).unwrap().request.source_uuid;
        let input = tmp.path().join("input/hooks/script");
        let once = AtomicBool::new(false);
        session.store.access_observer = Some(Arc::new(move |event| {
            if !matches!(event, IoEvent::Open) || once.swap(true, Ordering::SeqCst) {
                return;
            }
            match change {
                "update" => {
                    fs::write(&input, "changed").unwrap();
                    writer.update(claim, &Default::default()).unwrap();
                }
                "release" => {
                    writer.release(claim).unwrap();
                }
                "uninstall" => {
                    writer.uninstall(true, None).unwrap();
                }
                "credentials" => {
                    editor
                        .configure_application(
                            writer.app(),
                            ApplicationConfiguration {
                                expected_revision: editor
                                    .application(writer.app())
                                    .unwrap()
                                    .revision,
                                policy_refs: Default::default(),
                                credential: Some(
                                    crate::core::claim::registry::CredentialChange::Enable,
                                ),
                            },
                            None,
                        )
                        .unwrap();
                }
                _ => {
                    let record = PolicyRecord {
                        uuid: Uuid::new_v4(),
                        definition: PolicyDefinition::VerificationPolicy(
                            VerificationPolicy::Path {
                                mode: VerificationMode::Metadata,
                            },
                        ),
                    };
                    editor
                        .register_policies(std::slice::from_ref(&record))
                        .unwrap();
                    if change == "source-policy" {
                        editor
                            .adopt_source_policies(source, 0, vec![record.uuid].try_into().unwrap())
                            .unwrap();
                    }
                    if change == "app-policy" {
                        editor
                            .adopt_application_policies(
                                writer.app(),
                                editor.application(writer.app()).unwrap().revision,
                                vec![record.uuid].try_into().unwrap(),
                                None,
                            )
                            .unwrap();
                    }
                }
            }
        }));
        let result = session.lease_group(&request, &catalog);
        if change == "unrelated" {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err(), "{change}");
        }
        drop(result);
        assert!(available(&store, id), "{change}");
    }
}

#[test]
fn reversed_groups_and_reclaim_progress_while_another_group_scans() {
    use std::sync::{Mutex, mpsc};
    let (tmp, mut store, request, first) = fixture();
    let preparation = store.lease(first, VerificationMode::Metadata).unwrap();
    fs::write(tmp.path().join("input/hooks/script"), "second").unwrap();
    let second = store.acquire(&request).unwrap().entry.uuid;
    let mut request = group(first);
    request.members[1].target = GroupTarget::Output { uuid: second };
    let leaf = full();
    request.policy_refs = vec![leaf.uuid].try_into().unwrap();
    let catalog = PolicyCatalog::new([leaf]).unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let resume_rx = Mutex::new(resume_rx);
    let once = AtomicBool::new(false);
    store.access_observer = Some(Arc::new(move |event| {
        if matches!(event, IoEvent::Open) && !once.swap(true, Ordering::SeqCst) {
            ready_tx.send(()).unwrap();
            resume_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(10))
                .unwrap();
        }
    }));
    let scanner = store.clone();
    let scanning = request.clone();
    let cat = catalog.clone();
    let thread = std::thread::spawn(move || scanner.lease_group(&scanning, &cat));
    ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    store.access_observer = None;
    let (done_tx, done_rx) = mpsc::channel();
    let other = store.clone();
    let worker = std::thread::spawn(move || {
        request.members.reverse();
        let group = other.lease_group(&request, &catalog).unwrap();
        assert_eq!(group.output_count(), 2);
        other.path(second).unwrap();
        assert!(other.reclaim().unwrap().deferred.contains_key(&first));
        done_tx.send(()).unwrap();
    });
    let completed = done_rx.recv_timeout(Duration::from_secs(5));
    resume_tx.send(()).unwrap();
    drop(thread.join().unwrap().unwrap());
    worker.join().unwrap();
    completed.expect("group verification held global coordination");
    drop(preparation);
    assert!(available(&store, first) && available(&store, second));
    let mut invalid = group(first);
    invalid.members[1].target = GroupTarget::Output { uuid: second };
    invalid.members[1].relative = Some("missing".into());
    assert!(store.lease_group(&invalid, &Default::default()).is_err());
    assert!(available(&store, first) && available(&store, second));
}

#[test]
fn partial_marker_inheritance_and_spawn_failure_release_all_outputs() {
    for partial in [false, true] {
        let (tmp, store, request, first) = fixture();
        let preparation = store.lease(first, VerificationMode::Metadata).unwrap();
        fs::write(tmp.path().join("input/hooks/script"), "second").unwrap();
        let second = store.acquire(&request).unwrap().entry.uuid;
        let mut request = group(first);
        request.members[1].target = GroupTarget::Output { uuid: second };
        let mut owned = store.lease_group(&request, &Default::default()).unwrap();
        drop(preparation);
        if partial {
            owned.leases.values_mut().nth(1).unwrap().inherit().unwrap();
        }
        let result = crate::core::lease::launch::launch_group(
            std::process::Command::new(tmp.path().join("missing-program")),
            owned,
        );
        assert!(result.is_err());
        assert!(available(&store, first) && available(&store, second));
        assert!(!store.has_live_execution(first).unwrap());
        assert!(!store.has_live_execution(second).unwrap());
    }
}
