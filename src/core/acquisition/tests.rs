use super::*;
use std::fs;

fn setup() -> (tempfile::TempDir, Store, AppContext, Recipe) {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input");
    fs::create_dir_all(input.join("a")).unwrap();
    fs::create_dir_all(input.join("b")).unwrap();
    fs::write(input.join("a/file"), b"A").unwrap();
    fs::write(input.join("b/file"), b"B").unwrap();
    let store = Store::create_test(dir.path().join("store"), [7; 32]).unwrap();
    let token = store
        .register("a", AppSettings::default(), Filters::default())
        .unwrap();
    (
        dir,
        store,
        AppContext::authenticated(token),
        Recipe {
            source: Source::Local { path: input },
            folder: Some("a".into()),
            commit: None,
        },
    )
}

#[test]
fn local_acquisition_shares_sources_and_records_only_each_apps_touches() {
    let (dir, store, a, mut recipe) = setup();
    let b = AppContext::authenticated(
        store
            .register("b", AppSettings::default(), Filters::default())
            .unwrap(),
    );
    let first = store.acquire(&a, &recipe).unwrap();
    assert_eq!(fs::read(first.directory.join("file")).unwrap(), b"A");
    assert!(store.view(&b).unwrap().entries.is_empty());
    let reused = store.acquire(&b, &recipe).unwrap();
    assert_eq!(reused.artifact.id, first.artifact.id);
    recipe.folder = Some("b".into());
    let second = store.acquire(&a, &recipe).unwrap();
    assert_eq!(first.artifact.source_id, second.artifact.source_id);
    assert_eq!(first.artifact.snapshot_id, second.artifact.snapshot_id);
    assert_eq!(fs::read(second.directory.join("file")).unwrap(), b"B");
    assert_eq!(store.view(&a).unwrap().entries.len(), 2);
    assert_eq!(store.view(&b).unwrap().entries.len(), 1);
    assert!(
        store
            .artifact_path(&b, &second.artifact.id)
            .unwrap()
            .is_none()
    );
    store
        .configure(
            &a,
            AppSettings::default(),
            Filters {
                providers: ["git".into()].into(),
                ..Filters::default()
            },
        )
        .unwrap();
    assert!(store.view(&a).unwrap().entries.is_empty());
    assert!(
        store
            .artifact_path(&a, &first.artifact.id)
            .unwrap()
            .is_none()
    );
    assert_eq!(fs::read(dir.path().join("input/a/file")).unwrap(), b"A");
}

#[test]
fn current_and_five_historical_zips_roll_by_lru_and_preserve_live_content() {
    let (dir, store, a, recipe) = setup();
    let first = store.acquire(&a, &recipe).unwrap();
    let mut ids = vec![first.artifact.snapshot_id.clone()];
    for letter in b'B'..=b'F' {
        fs::write(dir.path().join("input/a/file"), [letter]).unwrap();
        ids.push(store.acquire(&a, &recipe).unwrap().artifact.snapshot_id);
    }
    let source = &first.artifact.source_id;
    let f = store.source_state(&a, source).unwrap().unwrap();
    assert_eq!(f.history.len(), 5);
    assert_eq!(f.current.as_ref().unwrap().id, ids[5]);
    store
        .read_snapshot(&a, source, &ids[0], Some("a".into()))
        .unwrap();
    fs::write(dir.path().join("input/a/file"), b"G").unwrap();
    let g = store.acquire(&a, &recipe).unwrap();
    let state = store.source_state(&a, source).unwrap().unwrap();
    assert_eq!(state.history.len(), 5);
    assert!(state.history.iter().any(|s| s.id == ids[0]));
    assert!(!state.history.iter().any(|s| s.id == ids[1]));
    assert_eq!(state.current.unwrap().id, g.artifact.snapshot_id);
    assert_eq!(fs::read(first.directory.join("file")).unwrap(), b"A");
    store.acquire(&a, &recipe).unwrap();
    assert_eq!(
        store
            .source_state(&a, source)
            .unwrap()
            .unwrap()
            .history
            .len(),
        5
    );
}

#[test]
fn retention_resumes_without_backfilling_and_verification_detects_edited_content() {
    let (dir, store, a, recipe) = setup();
    store
        .configure(
            &a,
            AppSettings {
                retain_snapshots: false,
                ..AppSettings::default()
            },
            Filters::default(),
        )
        .unwrap();
    let first = store.acquire(&a, &recipe).unwrap();
    let source = &first.artifact.source_id;
    assert!(
        store
            .source_state(&a, source)
            .unwrap()
            .unwrap()
            .current
            .unwrap()
            .zip_digest
            .is_none()
    );
    store
        .configure(&a, AppSettings::default(), Filters::default())
        .unwrap();
    store.acquire(&a, &recipe).unwrap();
    assert!(
        store
            .source_state(&a, source)
            .unwrap()
            .unwrap()
            .current
            .unwrap()
            .zip_digest
            .is_some()
    );
    store
        .configure(
            &a,
            AppSettings {
                retain_snapshots: false,
                ..AppSettings::default()
            },
            Filters::default(),
        )
        .unwrap();
    fs::write(dir.path().join("input/a/file"), b"unsaved").unwrap();
    let unsaved = store.acquire(&a, &recipe).unwrap();
    store
        .configure(
            &a,
            AppSettings {
                verify_content: true,
                ..AppSettings::default()
            },
            Filters::default(),
        )
        .unwrap();
    fs::write(dir.path().join("input/a/file"), b"next").unwrap();
    let next = store.acquire(&a, &recipe).unwrap();
    assert!(next.content_verified);
    let state = store.source_state(&a, source).unwrap().unwrap();
    assert!(
        !state
            .history
            .iter()
            .any(|s| s.id == unsaved.artifact.snapshot_id)
    );
    fs::write(next.directory.join("file"), b"tampered").unwrap();
    assert!(store.artifact_path(&a, &next.artifact.id).is_err());
    assert!(store.acquire(&a, &recipe).is_err());
}

#[test]
fn failures_preserve_state_and_mirrors_are_independent_copies() {
    let (dir, store, a, mut recipe) = setup();
    let first = store.acquire(&a, &recipe).unwrap();
    let before = store.source_state(&a, &first.artifact.source_id).unwrap();
    recipe.folder = Some("missing".into());
    assert!(store.acquire(&a, &recipe).is_err());
    assert_eq!(
        before,
        store.source_state(&a, &first.artifact.source_id).unwrap()
    );
    let mirror = dir.path().join("mirror");
    store.mirror(&a, &first.artifact.id, &mirror).unwrap();
    fs::write(mirror.join("file"), b"edited").unwrap();
    assert!(store.mirror(&a, &first.artifact.id, &mirror).is_err());
    assert_eq!(fs::read(mirror.join("file")).unwrap(), b"edited");
    assert_eq!(fs::read(first.directory.join("file")).unwrap(), b"A");
}

#[test]
fn verification_off_still_rejects_an_incomplete_cached_tree() {
    let (dir, store, app, recipe) = setup();
    let acquired = store.acquire(&app, &recipe).unwrap();
    fs::remove_file(acquired.directory.join("file")).unwrap();
    assert!(store.artifact_path(&app, &acquired.artifact.id).is_err());
    assert!(store.acquire(&app, &recipe).is_err());
    assert!(
        store
            .mirror(&app, &acquired.artifact.id, dir.path().join("mirror"))
            .is_err()
    );
    assert!(!dir.path().join("mirror").exists());
}

fn serve(bytes: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
    use std::io::{BufRead, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/download", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = std::io::BufReader::new(&mut stream);
        loop {
            let mut line = String::new();
            assert!(reader.read_line(&mut line).unwrap() > 0);
            if line == "\r\n" {
                break;
            }
        }
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        )
        .unwrap();
        stream.write_all(&bytes).unwrap();
    });
    (url, server)
}

#[test]
fn url_file_download_and_explicit_fallback_report_actual_cached_content() {
    let (_dir, store, app, _) = setup();
    let (url, server) = serve(b"downloaded".to_vec());
    let recipe = Recipe {
        source: Source::Url {
            url,
            download: Download::File {
                name: "payload.txt".into(),
            },
        },
        folder: None,
        commit: None,
    };
    let first = store.acquire(&app, &recipe).unwrap();
    server.join().unwrap();
    assert_eq!(
        fs::read(first.directory.join("payload.txt")).unwrap(),
        b"downloaded"
    );
    assert!(!first.fallback);
    assert!(first.update_checked);
    let before = store
        .source_state(&app, &first.artifact.source_id)
        .unwrap()
        .unwrap();
    assert!(store.acquire(&app, &recipe).is_err());
    store
        .configure(
            &app,
            AppSettings {
                allow_local_fallback: true,
                verify_content: true,
                ..AppSettings::default()
            },
            Filters::default(),
        )
        .unwrap();
    let reused = store.acquire(&app, &recipe).unwrap();
    assert!(reused.fallback && !reused.update_checked && reused.content_verified);
    assert_eq!(reused.artifact, first.artifact);
    let after = store
        .source_state(&app, &first.artifact.source_id)
        .unwrap()
        .unwrap();
    assert_eq!(before.history, after.history);
    assert_eq!(before.current.unwrap().id, after.current.unwrap().id);
    fs::write(reused.directory.join("payload.txt"), b"tampered").unwrap();
    assert!(store.acquire(&app, &recipe).is_err());
    store
        .configure(
            &app,
            AppSettings {
                allow_local_fallback: true,
                ..AppSettings::default()
            },
            Filters::default(),
        )
        .unwrap();
    fs::remove_file(reused.directory.join("payload.txt")).unwrap();
    assert!(
        store.acquire(&app, &recipe).is_err(),
        "missing fallback files must fail even without content hashing"
    );
}

#[test]
fn url_zip_extracts_content_and_invalid_archives_do_not_publish() {
    let (dir, store, app, _) = setup();
    let bytes = crate::utils::archive::write_zip(
        dir.path().join("input"),
        std::io::Cursor::new(Vec::new()),
        |_, _| true,
    )
    .unwrap()
    .into_inner();
    let (url, server) = serve(bytes);
    let recipe = Recipe {
        source: Source::Url {
            url,
            download: Download::Zip,
        },
        folder: Some("a".into()),
        commit: None,
    };
    let result = store.acquire(&app, &recipe).unwrap();
    server.join().unwrap();
    assert_eq!(fs::read(result.directory.join("file")).unwrap(), b"A");
    let (url, server) = serve(b"invalid archive".to_vec());
    let bad = Recipe {
        source: Source::Url {
            url,
            download: Download::Zip,
        },
        folder: None,
        commit: None,
    };
    let before = store.view(&app).unwrap();
    assert!(store.acquire(&app, &bad).is_err());
    server.join().unwrap();
    assert_eq!(before, store.view(&app).unwrap());
}

#[test]
fn source_indexes_are_encrypted_and_tampering_is_never_a_cache_fallback() {
    let (dir, store, app, recipe) = setup();
    let acquired = store.acquire(&app, &recipe).unwrap();
    let root = dir
        .path()
        .join("store/sources")
        .join(&acquired.artifact.source_id);
    let indexes = fs::read_dir(root.join("indexes"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(indexes.len(), 1);
    let path = indexes[0].path();
    let mut bytes = fs::read(&path).unwrap();
    assert!(!bytes.windows(5).any(|b| b == b"local"));
    *bytes.last_mut().unwrap() ^= 1;
    fs::write(&path, bytes).unwrap();
    assert!(store.acquire(&app, &recipe).is_err());
    assert!(store.artifact_path(&app, &acquired.artifact.id).is_err());
}

#[test]
fn unavailable_outgoing_zip_cannot_be_silently_lost_during_rollover() {
    let (dir, store, app, recipe) = setup();
    let first = store.acquire(&app, &recipe).unwrap();
    let before = store.source_state(&app, &first.artifact.source_id).unwrap();
    let zip = store
        .root
        .join("sources")
        .join(&first.artifact.source_id)
        .join("snapshots")
        .join(format!("{}.zip", first.artifact.snapshot_id));
    fs::remove_file(zip).unwrap();
    fs::write(dir.path().join("input/a/file"), b"next").unwrap();
    assert!(store.acquire(&app, &recipe).is_err());
    assert_eq!(
        before,
        store.source_state(&app, &first.artifact.source_id).unwrap()
    );
}

#[test]
fn concurrent_apps_publish_consistent_shared_content_without_lost_touches() {
    let (_dir, store, a, recipe) = setup();
    let b = AppContext::authenticated(
        store
            .register("b", AppSettings::default(), Filters::default())
            .unwrap(),
    );
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = [a.clone(), b.clone()]
        .into_iter()
        .map(|context| {
            let store = Store::open_test(&store.root, [7; 32]).unwrap();
            let recipe = recipe.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.acquire(&context, &recipe).unwrap()
            })
        })
        .collect();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(results[0].artifact.id, results[1].artifact.id);
    assert_eq!(store.view(&a).unwrap().entries.len(), 1);
    assert_eq!(store.view(&b).unwrap().entries.len(), 1);
    assert!(
        store
            .source_state(&a, &results[0].artifact.source_id)
            .unwrap()
            .unwrap()
            .history
            .is_empty()
    );
}

#[test]
fn failed_index_publication_keeps_the_old_selection_and_allows_retry() {
    let (dir, store, app, recipe) = setup();
    let first = store.acquire(&app, &recipe).unwrap();
    let before = store.source_state(&app, &first.artifact.source_id).unwrap();
    let view = store.view(&app).unwrap();
    let metadata_dir = store
        .root
        .join("sources")
        .join(&first.artifact.source_id)
        .join("indexes");
    let old_metadata = fs::read_dir(&metadata_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(dir.path().join("input/a/file"), b"new").unwrap();
    store
        .fail_publication
        .store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(store.acquire(&app, &recipe).is_err());
    assert_eq!(
        store.source_state(&app, &first.artifact.source_id).unwrap(),
        before
    );
    assert_eq!(store.view(&app).unwrap(), view);
    assert_eq!(fs::read(first.directory.join("file")).unwrap(), b"A");
    store
        .fail_publication
        .store(false, std::sync::atomic::Ordering::Relaxed);
    let orphan = fs::read_dir(&metadata_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|path| path != &old_metadata)
        .unwrap();
    let bytes = fs::read(&orphan).unwrap();
    fs::write(&orphan, b"corrupt uncommitted source index").unwrap();
    assert!(store.acquire(&app, &recipe).is_err());
    assert_eq!(store.view(&app).unwrap(), view);
    fs::write(&orphan, bytes).unwrap();
    let retried = store.acquire(&app, &recipe).unwrap();
    assert_eq!(fs::read(retried.directory.join("file")).unwrap(), b"new");
}
