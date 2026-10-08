use super::*;
use crate::{
    models::{
        policy::{Eviction, Identical},
        source::Source,
        store::{Request, Selector},
    },
    utils::tree::IoEvent,
};
use std::sync::{Arc, Mutex, mpsc};

#[test]
fn observed_io_and_paused_scan_do_not_serialize_unrelated_operations() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("data");
    fs::write(&input, b"payload").unwrap();
    let mut store = Store::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let mut request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    request.policies.identical = Identical::NewEntry;
    request.policies.eviction = Eviction::Latest { keep: 1 };
    let first = store.acquire(&request).unwrap();
    let counts = Arc::new(Mutex::new((0, 0, 0)));
    let seen = counts.clone();
    store.access_observer = Some(Arc::new(move |event| {
        let mut c = seen.lock().unwrap();
        match event {
            IoEvent::Directory => c.0 += 1,
            IoEvent::Open => c.1 += 1,
            IoEvent::Bytes(n) => c.2 += n,
        }
    }));
    store.path(first.entry.uuid).unwrap();
    assert_eq!(*counts.lock().unwrap(), (0, 0, 0));
    store.verify(first.entry.uuid).unwrap();
    assert_eq!(*counts.lock().unwrap(), (1, 1, 7));
    eprintln!(
        "access payload IO: metadata=0 directories/0 opens/0 bytes; full=1 directory/1 open/7 bytes"
    );
    let (paused_tx, paused_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let resume_rx = Mutex::new(resume_rx);
    store.access_observer = Some(Arc::new(move |event| {
        if matches!(event, IoEvent::Open) {
            paused_tx.send(()).unwrap();
            resume_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(10))
                .unwrap();
        }
    }));
    let worker = store.clone();
    let thread = std::thread::spawn(move || worker.verify(first.entry.uuid));
    paused_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    store.access_observer = None;
    let (completed_tx, completed_rx) = mpsc::channel();
    let other = store.clone();
    let work = std::thread::spawn(move || {
        let next = other.acquire(&request).unwrap();
        other.path(next.entry.uuid).unwrap();
        other.register_app("unrelated mutation".into()).unwrap();
        assert!(
            other
                .reclaim()
                .unwrap()
                .deferred
                .contains_key(&first.entry.uuid)
        );
        completed_tx.send(()).unwrap();
    });
    let completed = completed_rx.recv_timeout(Duration::from_secs(5));
    resume_tx.send(()).unwrap();
    thread.join().unwrap().unwrap();
    work.join().unwrap();
    completed.expect("unrelated operations waited for payload scan");
    assert!(
        store
            .reclaim()
            .unwrap()
            .reclaimed
            .contains(&first.entry.uuid)
    );
}

#[test]
fn managed_launch_boundary_checks_target_without_payload_io() {
    use crate::models::index::{Binding, Record};
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("tool");
    fs::write(&input, b"not executed at this boundary").unwrap();
    let mut store = Store::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let mut request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    request.selection.bin.push("tool".into());
    let output = store.acquire(&request).unwrap();
    let mut app = store.register_app("binding".into()).unwrap();
    app.active = Some(Binding {
        entry_uuid: output.entry.uuid,
        executable: "tool".into(),
    });
    store
        .save_record(&Record::Application(app.clone()))
        .unwrap();
    store.access_observer = Some(Arc::new(|_| panic!("managed launch scanned payload")));
    let (command, lease) = store
        .prepare_launch(app.uuid, &["literal $value".into()], true)
        .unwrap();
    assert_eq!(lease.resource().verification, VerificationMode::Metadata);
    assert_eq!(command.get_program(), output.directory.join("tool"));
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        vec![
            std::ffi::OsStr::new("--root"),
            store.root().as_os_str(),
            std::ffi::OsStr::new("literal $value")
        ]
    );
    assert!(
        store
            .prepare_launch(
                app.uuid,
                &["--root".into(), tmp.path().as_os_str().into()],
                true
            )
            .is_err()
    );
    drop(lease);
    for bad in ["../outside", "undeclared"] {
        app.active.as_mut().unwrap().executable = bad.into();
        store
            .save_record(&Record::Application(app.clone()))
            .unwrap();
        assert!(store.prepare_launch(app.uuid, &[], false).is_err());
    }
    app.active.as_mut().unwrap().executable = "tool".into();
    store
        .save_record(&Record::Application(app.clone()))
        .unwrap();
    fs::remove_file(output.directory.join("tool")).unwrap();
    assert!(store.prepare_launch(app.uuid, &[], false).is_err());
    fs::create_dir(output.directory.join("tool")).unwrap();
    assert!(store.prepare_launch(app.uuid, &[], false).is_err());
    eprintln!("managed launch boundary payload IO: 0 directories/0 opens/0 bytes");
}

#[test]
fn activation_scans_once_per_phase_outside_mutation_lock() {
    use crate::{core::activation::Probe, models::policy::Protection};
    let tmp = tempfile::tempdir().unwrap();
    let exe = std::env::current_exe().unwrap();
    let name = exe.file_name().unwrap().to_str().unwrap().to_owned();
    let mut store = Store::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: exe.clone() })
        .unwrap();
    let mut request = Request {
        source_uuid: source.uuid,
        selector: Selector::Local,
        selection: Default::default(),
        policies: Default::default(),
    };
    request.selection.bin.push(name.clone());
    request.policies.protection = Some(Protection::UseableApp);
    let acquired = store.acquire(&request).unwrap();
    let app = store.register_app("observed activation".into()).unwrap();
    let counts = Arc::new(Mutex::new((0, 0, 0)));
    let seen = counts.clone();
    let root = store.root().to_path_buf();
    store.access_observer = Some(Arc::new(move |event| {
        let mut c = seen.lock().unwrap();
        match event {
            IoEvent::Directory => {
                c.0 += 1;
                let _mutation = LockGuard::acquire(
                    &root.join(".locks/mutation"),
                    LockMode::Exclusive,
                    Duration::ZERO,
                )
                .expect("activation scan held mutation lock");
                assert!(
                    LockGuard::acquire(
                        &root.join(format!(".locks/use-{}", acquired.entry.uuid)),
                        LockMode::Exclusive,
                        Duration::ZERO
                    )
                    .is_err()
                );
            }
            IoEvent::Open => c.1 += 1,
            IoEvent::Bytes(n) => c.2 += n,
        }
    }));
    store
        .activate(
            app.uuid,
            acquired.entry.uuid,
            &name,
            &Probe {
                args: vec!["--list".into()],
                stdout_contains: "core::store".into(),
                timeout_seconds: 10,
            },
        )
        .unwrap();
    assert_eq!(
        *counts.lock().unwrap(),
        (2, 2, 2 * fs::metadata(exe).unwrap().len() as usize)
    );
}

#[test]
fn sweep_preserves_live_markers_and_creation_windows_on_retained_outputs() {
    use crate::models::index::{Binding, Record};
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("data");
    fs::write(&input, b"retained").unwrap();
    let store = Store::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Local { path: input })
        .unwrap();
    let acquired = store
        .acquire(&Request {
            source_uuid: source.uuid,
            selector: Selector::Local,
            selection: Default::default(),
            policies: Default::default(),
        })
        .unwrap();
    let id = acquired.entry.uuid;
    let before = fs::read(store.root().join(format!("index/{id}/index.json"))).unwrap();
    let locks = store.root().join(".locks");
    let unknown = locks.join("live-not-a-saucepan-marker");
    fs::write(&unknown, b"preserve").unwrap();
    let unrelated_dir = locks.join(format!("live-{id}-{}", Uuid::new_v4()));
    fs::create_dir(&unrelated_dir).unwrap();
    for active in [false, true] {
        if active {
            let mut app = store.register_app("active retained entry".into()).unwrap();
            app.active = Some(Binding {
                entry_uuid: id,
                executable: "data".into(),
            });
            store.save_record(&Record::Application(app)).unwrap();
        }
        let closed = locks.join(format!("live-{id}-{}", Uuid::new_v4()));
        fs::write(&closed, b"").unwrap();
        let live = locks.join(format!("live-{id}-{}", Uuid::new_v4()));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(3);
        }
        let held = options.open(&live).unwrap();
        #[cfg(unix)]
        held.lock_shared().unwrap();
        store.reclaim().unwrap();
        assert!(!closed.exists());
        assert!(live.exists());
        drop(held);
        store.reclaim().unwrap();
        assert!(!live.exists());

        // Simulate the exact pre-lock creation window: shared coordination is
        // already held but the new marker has not yet acquired native ownership.
        let guard = store.lease(id, VerificationMode::Metadata).unwrap();
        let creating = guard.marker_path.clone();
        fs::write(&creating, b"").unwrap();
        store.reclaim().unwrap();
        assert!(creating.exists(), "sweep raced marker initialization");
        drop(guard);
        store.reclaim().unwrap();
        assert!(!creating.exists());
    }
    assert_eq!(fs::read(unknown).unwrap(), b"preserve");
    assert!(unrelated_dir.is_dir());
    assert!(locks.join("mutation").exists());
    assert!(locks.join(format!("use-{id}")).exists());
    assert_eq!(
        fs::read(store.root().join(format!("index/{id}/index.json"))).unwrap(),
        before
    );
    assert_eq!(
        fs::read(acquired.directory.join("data")).unwrap(),
        b"retained"
    );
}
