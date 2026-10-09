mod native_root;
use saucepan::{
    core::{
        Store,
        activation::Probe,
        claim::{
            persistence::{Metadata, Publication},
            registry::CredentialChange,
            security::{Identity, Kind, NativeKeyring},
            session::{ApplicationConfiguration, ApplicationRegistration},
        },
        query::{ClaimQuery, DiscoveryQuery},
    },
    error::{Error, ErrorCode},
    models::{
        claim::{
            EnsureRequest,
            policy::{Action, ReclaimPolicy, ReclaimStrategy},
        },
        index::lifecycle::{Installation, Phase},
        policy::{Eviction, Identical, Protection},
        source::Source,
        store::{Availability, Platform, Request, Selector, VerificationMode},
    },
    utils::{
        lock::{LockGuard, LockMode},
        tree::digest,
    },
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
fn hash(path: &Path) -> String {
    digest(fs::File::open(path).unwrap()).unwrap()
}
fn mock_snapshot() -> BTreeMap<String, String> {
    fn walk(root: &Path, path: &Path, out: &mut BTreeMap<String, String>) {
        for item in fs::read_dir(path).unwrap() {
            let path = item.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().to_string_lossy().into(),
                    hash(&path),
                );
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("mock");
    let mut files = BTreeMap::new();
    if root.exists() {
        walk(&root, &root, &mut files);
    }
    files
}
fn code(error: anyhow::Error) -> ErrorCode {
    Error::from_anyhow(&error).code
}

#[test]
fn fresh_root_cross_feature_acceptance_with_real_binaries() {
    let mock = mock_snapshot();
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let input = tmp.path().join("packages");
    fs::create_dir_all(input.join("tool")).unwrap();
    fs::write(input.join("tool/resource"), "original").unwrap();
    let source = store
        .register_source(Source::Local {
            path: input.clone(),
        })
        .unwrap();
    let mut registration = ApplicationRegistration {
        registration_key: "consumer".to_string().try_into().unwrap(),
        name: "consumer".into(),
        access: Default::default(),
        policies: Default::default(),
        credentials_required: false,
    };
    registration.access.sources = Some([source.uuid].into());
    registration
        .access
        .public_scopes
        .insert("public".to_string().try_into().unwrap());
    let app = store
        .register_application(&registration, None)
        .unwrap()
        .application
        .uuid;
    let session = store.session(app, None).unwrap();
    let mut request = EnsureRequest {
        scope: "public".to_string().try_into().unwrap(),
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        context: [("package".into(), "tool".into())].into(),
        overrides: Default::default(),
    };
    request.overrides.keep = Some(1);
    request.overrides.reclaim = Some(ReclaimPolicy::ReclaimPolicy {
        strategy: ReclaimStrategy::RestoreExact,
    });
    let first = session.ensure(&request, &Default::default()).unwrap();
    assert_eq!(
        session
            .ensure(&request, &Default::default())
            .unwrap()
            .claim
            .uuid,
        first.claim.uuid
    );
    request.selection.folder = Some("tool".into());
    let covered = session.ensure(&request, &Default::default()).unwrap();
    assert_eq!(covered.claim.uuid, first.claim.uuid);
    request.selection.folder = None;
    request.scope = "private".to_string().try_into().unwrap();
    request.overrides.reclaim = Some(ReclaimPolicy::ReclaimPolicy {
        strategy: ReclaimStrategy::Refresh,
    });
    let private = session.ensure(&request, &Default::default()).unwrap();
    assert_eq!(first.claim.entry_uuid, private.claim.entry_uuid);
    let found = store
        .discover_claims(&DiscoveryQuery {
            scope: None,
            conditions: vec![],
        })
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].claims.len(), 1);
    assert_eq!(found[0].claims[0].claim.uuid, first.claim.uuid);
    assert_eq!(
        session
            .query(&ClaimQuery {
                app,
                scope: Some(private.claim.scope.clone()),
                conditions: vec![]
            })
            .unwrap()
            .claims
            .len(),
        1
    );
    let outsider = store.register_app("outsider".into()).unwrap().uuid;
    assert_eq!(
        code(
            store
                .session(outsider, None)
                .unwrap()
                .lease_output(first.claim.entry_uuid, VerificationMode::Full)
                .err()
                .unwrap()
        ),
        ErrorCode::AccessDenied
    );
    let forbidden_source = store
        .register_source(Source::Local {
            path: tmp.path().to_owned(),
        })
        .unwrap();
    let mut forbidden = request.clone();
    forbidden.source_uuid = forbidden_source.uuid;
    assert_eq!(
        code(session.ensure(&forbidden, &Default::default()).unwrap_err()),
        ErrorCode::AccessDenied
    );
    let held = session
        .lease_view(&covered.view, VerificationMode::Full)
        .unwrap();
    let mut extra = first.claim.request.clone();
    extra.policies.identical = Identical::NewEntry;
    store.acquire(&extra).unwrap();
    store.reclaim().unwrap();
    assert_eq!(
        store.output(first.claim.entry_uuid).unwrap().state,
        Availability::Ready
    );
    assert_eq!(
        fs::read(held.directory().join("resource")).unwrap(),
        b"original"
    );
    drop(held);
    store.reclaim().unwrap();
    assert_eq!(
        store.output(first.claim.entry_uuid).unwrap().state,
        Availability::Reclaimed
    );
    let exact = session
        .recover(first.claim.uuid, &Default::default())
        .unwrap();
    assert_ne!(exact.claim.entry_uuid, first.claim.entry_uuid);
    assert_eq!(exact.acquired.entry.files, first.acquired.entry.files);
    fs::write(input.join("tool/resource"), "new").unwrap();
    let refresh = session
        .recover(private.claim.uuid, &Default::default())
        .unwrap();
    assert_eq!(
        fs::read(refresh.acquired.directory.join("tool/resource")).unwrap(),
        b"new"
    );
    let member = store
        .root()
        .join(format!("claims/{app}/{}.json", first.claim.uuid));
    let bytes = fs::read(&member).unwrap();
    fs::write(
        &member,
        String::from_utf8_lossy(&bytes).replace("tool", "edited"),
    )
    .unwrap();
    assert_eq!(
        code(session.claims().unwrap_err()),
        ErrorCode::IntegrityError
    );
    fs::write(&member, bytes).unwrap();
    let proof = store
        .configure_application(
            app,
            ApplicationConfiguration {
                expected_revision: store.application(app).unwrap().revision,
                access: registration.access.clone(),
                policies: registration.policies.clone(),
                credential: Some(CredentialChange::Enable),
            },
            None,
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        code(store.session(app, None).err().unwrap()),
        ErrorCode::AuthenticationRequired
    );
    let protected = store.session(app, Some(proof)).unwrap();
    assert_eq!(protected.claims().unwrap().len(), 2);
    assert!(
        store
            .discover_claims(&DiscoveryQuery {
                scope: None,
                conditions: vec![]
            })
            .unwrap()
            .is_empty()
    );

    // One authenticated crash/replay transition in this same root.
    {
        let _lock = LockGuard::acquire(
            &store.root().join(".locks/mutation"),
            LockMode::Exclusive,
            Duration::from_secs(10),
        )
        .unwrap();
        let metadata = Metadata::open(store.root(), &NativeKeyring).unwrap();
        let mut updated_source = source.clone();
        updated_source.refresh.ttl_seconds = 43200;
        let interrupted = metadata.publish_observed(
            &metadata.snapshot().unwrap(),
            vec![(
                Identity {
                    kind: Kind::Source,
                    owner: None,
                    member: Some(source.uuid),
                },
                serde_json::to_value(updated_source).unwrap(),
            )],
            vec![],
            |point| {
                if point == Publication::Journal {
                    Err(Error::new(ErrorCode::IoError, "acceptance interruption"))
                } else {
                    Ok(())
                }
            },
        );
        assert!(interrupted.is_err());
    }
    let reopened = Store::open(store.root()).unwrap();
    assert_eq!(
        reopened.source(source.uuid).unwrap().refresh.ttl_seconds,
        43200
    );
    assert!(
        !store
            .root()
            .join(".staging/metadata-transaction.json")
            .exists()
    );

    let binary = PathBuf::from(env!("CARGO_BIN_EXE_saucepan"));
    let launcher = PathBuf::from(env!("CARGO_BIN_EXE_saucepan-shim"));
    let executable = binary.file_name().unwrap().to_str().unwrap().to_owned();
    let binary_source = store
        .register_source(Source::Local {
            path: binary.clone(),
        })
        .unwrap();
    let mut binary_request = Request {
        source_uuid: binary_source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    binary_request.selection.bin = vec![executable.clone()];
    binary_request.selection.platform = Some(Platform::host());
    binary_request.policies.identical = Identical::NewEntry;
    binary_request.policies.eviction = Eviction::Latest { keep: 2 };
    binary_request.policies.protection = Some(Protection::UseableApp);
    let probe = Probe {
        args: vec!["--version".into()],
        stdout_contains: "saucepan ".into(),
        timeout_seconds: 10,
    };
    let installed = store
        .install_application(
            &Installation {
                scope: "saucepan/self".to_string().try_into().unwrap(),
                name: "saucepan".into(),
                request: binary_request,
                probe: probe.clone(),
                shim: executable.clone(),
                saucepan: true,
            },
            &launcher,
            &Default::default(),
            None,
        )
        .unwrap();
    assert_eq!(installed.status, Phase::Complete, "{:?}", installed.pending);
    let original_binding = installed.candidate.unwrap();
    let token = store
        .configure_application(
            installed.app,
            ApplicationConfiguration {
                expected_revision: store.application(installed.app).unwrap().revision,
                access: Default::default(),
                policies: Default::default(),
                credential: Some(CredentialChange::Enable),
            },
            None,
        )
        .unwrap()
        .unwrap();
    let managed = store.session(installed.app, Some(token)).unwrap();
    let shim_output = Command::new(store.root().join("bin").join(&executable))
        .arg("--version")
        .output()
        .unwrap();
    assert!(
        shim_output.status.success(),
        "{}",
        String::from_utf8_lossy(&shim_output.stderr)
    );
    let update = managed
        .update_application(installed.claim, &probe, None, &Default::default(), None)
        .unwrap();
    assert_eq!(update.status, Phase::Complete);
    assert_ne!(update.candidate, Some(original_binding));
    let rollback = managed
        .update_application(
            installed.claim,
            &probe,
            Some(original_binding),
            &Default::default(),
            None,
        )
        .unwrap();
    assert_eq!(rollback.status, Phase::Complete);
    assert_eq!(
        store
            .application(installed.app)
            .unwrap()
            .active
            .unwrap()
            .entry_uuid,
        original_binding
    );
    let removed = managed.uninstall(false, None).unwrap();
    assert_eq!(removed.status, Phase::Complete);
    assert!(
        !store
            .root()
            .join(format!("claims/{}", installed.app))
            .exists()
    );
    let retention = store.reclaim().unwrap();
    assert_eq!(mock_snapshot(), mock);
    let report = json!({"host":Platform::host(),"version":env!("CARGO_PKG_VERSION"),"build":env!("SAUCEPAN_BUILD_ID"),
        "binary_sha256":hash(&binary),"shim_sha256":hash(&launcher),"app":app,"claims":[first.claim.uuid,private.claim.uuid],
        "outputs":{"initial":first.claim.entry_uuid,"exact":exact.claim.entry_uuid,"refresh":refresh.claim.entry_uuid},
        "managed":{"app":installed.app,"original":original_binding,"updated":update.candidate,"rolled_back":rollback.candidate,"uninstall":removed.status},
        "errors":[ErrorCode::AccessDenied,ErrorCode::IntegrityError,ErrorCode::AuthenticationRequired],"source_replay_ttl":43200,
        "retention":retention,"mock_files_unchanged":mock.len(),"permitted_action_example":Action::Read});
    if let Some(path) = std::env::var_os("SAUCEPAN_ACCEPTANCE_REPORT") {
        fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    println!("ACCEPTANCE:{report}");
}
