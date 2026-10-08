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
