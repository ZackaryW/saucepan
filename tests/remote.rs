mod native_root;
mod policy_fixture;
mod support;
use saucepan::{
    core::source::AcquireOptions,
    models::{
        source::Source,
        store::{AssetFilter, Format, Request, Selector},
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
#[test]
fn release_cache_expiry_fallback_and_cli_use_real_transport() {
    let hits = Arc::new(AtomicUsize::new(0));
    let downloads = Arc::new(AtomicUsize::new(0));
    let fail = Arc::new(AtomicBool::new(false));
    let version = Arc::new(AtomicUsize::new(1));
    let h = hits.clone();
    let d = downloads.clone();
    let f = fail.clone();
    let v = version.clone();
    let server = support::Server::new(move |path, base| {
        if path.starts_with("/repos/") {
            h.fetch_add(1, Ordering::SeqCst);
            if f.load(Ordering::SeqCst) {
                return (503, b"unavailable".to_vec());
            }
            let n = v.load(Ordering::SeqCst);
            (200,serde_json::json!({"id":n,"tag_name":format!("v{n}"),"draft":false,"prerelease":false,"assets":[{"id":n,"name":"tool.exe","browser_download_url":format!("{base}/asset/{n}"),"digest":null}]}).to_string().into_bytes())
        } else {
            d.fetch_add(1, Ordering::SeqCst);
            (200, path.as_bytes().to_vec())
        }
    });
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::GithubRelease {
            repo: "owner/tool".into(),
        })
        .unwrap();
    let mut request = Request {
        source_uuid: source.uuid,
        selector: Selector::LatestStable,
        selection: Default::default(),
        policy_refs: Default::default(),
    };
    request.selection.format = Format::Raw;
    request.selection.filter_refs = policy_fixture::filters(
        &store,
        vec![AssetFilter {
            platform: None,
            asset_patterns: vec!["*.exe".into()],
        }],
    );
    let mut opts = AcquireOptions {
        test_now: Some(100),
        test_github_api: Some(server.base.clone()),
        ..Default::default()
    };
    let first = store.acquire_with(&request, &opts).unwrap();
    assert!(first.update_checked);
    opts.test_now = Some(86499);
    assert!(store.acquire_with(&request, &opts).unwrap().reused);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert_eq!(downloads.load(Ordering::SeqCst), 1);
    opts.test_now = Some(86500);
    version.store(2, Ordering::SeqCst);
    let second = store.acquire_with(&request, &opts).unwrap();
    assert_ne!(first.entry.uuid, second.entry.uuid);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    assert_eq!(
        std::fs::read(first.directory.join("tool.exe")).unwrap(),
        b"/asset/1"
    );
    fail.store(true, Ordering::SeqCst);
    opts.force_refresh = true;
    assert!(store.acquire_with(&request, &opts).is_err());
    opts.allow_stale = true;
    let stale = store.acquire_with(&request, &opts).unwrap();
    assert!(stale.stale);
    assert_eq!(stale.entry.uuid, second.entry.uuid);
    assert_eq!(
        store
            .source(source.uuid)
            .unwrap()
            .discovery
            .values()
            .next()
            .unwrap()
            .checked_at,
        86500
    );
    fail.store(false, Ordering::SeqCst);
    opts.force_refresh = false;
    opts.test_now = Some(1);
    assert!(store.acquire_with(&request, &opts).unwrap().update_checked);
    assert!(
        store
            .root()
            .join("source")
            .join(source.uuid.to_string())
            .join("retrieval")
            .is_dir()
    );
    let file = tmp.path().join("release-request.json");
    std::fs::write(&file, serde_json::to_vec(&request).unwrap()).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .arg("--root")
        .arg(store.root())
        .args([
            "--test-github-api",
            &server.base,
            "--test-now",
            "2",
            "acquire",
        ])
        .arg(file)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn filters_do_not_choose_arbitrary_assets() {
    use saucepan::{
        core::policy::select_asset,
        models::{
            source::Asset,
            store::{Platform, Selection},
        },
    };
    let assets = vec![
        Asset {
            id: 1,
            name: "a.zip".into(),
            url: "https://example.test/a".into(),
            digest: None,
        },
        Asset {
            id: 2,
            name: "b.zip".into(),
            url: "https://example.test/b".into(),
            digest: None,
        },
    ];
    let mut selection = Selection {
        filters: vec![AssetFilter {
            platform: None,
            asset_patterns: vec!["*.zip".into()],
        }],
        ..Default::default()
    };
    assert!(select_asset(&assets, &selection).is_err());
    selection.filters[0].asset_patterns = vec!["missing".into()];
    assert!(select_asset(&assets, &selection).is_err());
    selection.filters.push(AssetFilter {
        platform: Some(Platform::host()),
        asset_patterns: vec!["b.zip".into()],
    });
    assert_eq!(select_asset(&assets, &selection).unwrap().id, 2);
}

#[test]
fn url_downloads_refresh_unvalidated_bytes_and_preserve_previous_content() {
    let version = Arc::new(AtomicUsize::new(1));
    let v = version.clone();
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    let server = support::Server::new(move |_, _| {
        h.fetch_add(1, Ordering::SeqCst);
        (
            200,
            format!("version {}", v.load(Ordering::SeqCst)).into_bytes(),
        )
    });
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Url {
            url: format!("{}/file", server.base),
        })
        .unwrap();
    let mut request = Request {
        source_uuid: source.uuid,
        selector: Selector::Url,
        selection: Default::default(),
        policy_refs: Default::default(),
    };
    request.selection.format = Format::Raw;
    request.selection.filename = Some("payload".into());
    let mut opts = AcquireOptions {
        test_now: Some(100),
        ..Default::default()
    };
    let first = store.acquire_with(&request, &opts).unwrap();
    let initial = hits.load(Ordering::SeqCst);
    version.store(2, Ordering::SeqCst);
    assert!(store.acquire_with(&request, &opts).unwrap().reused);
    assert_eq!(hits.load(Ordering::SeqCst), initial);
    opts.test_now = Some(86500);
    let second = store.acquire_with(&request, &opts).unwrap();
    assert_ne!(first.entry.uuid, second.entry.uuid);
    assert_eq!(
        std::fs::read(first.directory.join("payload")).unwrap(),
        b"version 1"
    );
    assert_eq!(
        std::fs::read(second.directory.join("payload")).unwrap(),
        b"version 2"
    );
}

#[test]
fn zip_integrity_tag_and_shared_platform_discovery() {
    use saucepan::{
        models::{index::Record, store::Platform},
        utils::tree::digest,
    };
    use std::io::{Cursor, Write};
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        "package/index.json",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(b"ordinary content").unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    let expected = digest(Cursor::new(&bytes)).unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    let corrupt = Arc::new(AtomicBool::new(false));
    let c = corrupt.clone();
    let server = support::Server::new(move |path, base| {
        if path.starts_with("/repos/") {
            assert!(path.ends_with("/releases/tags/v1"));
            h.fetch_add(1, Ordering::SeqCst);
            (200, serde_json::json!({"id":1,"tag_name":"v1","draft":false,"prerelease":false,"assets":[
                {"id":1,"name":"windows.zip","browser_download_url":format!("{base}/windows"),"digest":format!("sha256:{expected}")},
                {"id":2,"name":"linux.zip","browser_download_url":format!("{base}/linux"),"digest":format!("sha256:{expected}")}
            ]}).to_string().into_bytes())
        } else if c.load(Ordering::SeqCst) {
            (200, b"corrupt".to_vec())
        } else {
            (200, bytes.clone())
        }
    });
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::GithubRelease {
            repo: "owner/tool".into(),
        })
        .unwrap();
    let mut request = Request {
        source_uuid: source.uuid,
        selector: Selector::Tag { tag: "v1".into() },
        selection: Default::default(),
        policy_refs: Default::default(),
    };
    request.selection.format = Format::Zip;
    request.selection.folder = Some("package".into());
    request.selection.filter_refs = policy_fixture::filters(
        &store,
        ["windows", "linux"]
            .into_iter()
            .map(|os| AssetFilter {
                platform: Some(Platform {
                    os: os.into(),
                    arch: "x86_64".into(),
                }),
                asset_patterns: vec![format!("{os}.zip")],
            })
            .collect::<Vec<_>>(),
    );
    request.selection.platform = Some(Platform {
        os: "windows".into(),
        arch: "x86_64".into(),
    });
    let opts = AcquireOptions {
        test_now: Some(100),
        test_github_api: Some(server.base.clone()),
        ..Default::default()
    };
    let first = store.acquire_with(&request, &opts).unwrap();
    assert_eq!(
        std::fs::read(first.directory.join("index.json")).unwrap(),
        b"ordinary content"
    );
    request.selection.platform.as_mut().unwrap().os = "linux".into();
    corrupt.store(true, Ordering::SeqCst);
    assert!(store.acquire_with(&request, &opts).is_err());
    assert_eq!(
        store
            .records()
            .unwrap()
            .iter()
            .filter(|r| matches!(r, Record::Output(_)))
            .count(),
        1
    );
    corrupt.store(false, Ordering::SeqCst);
    let second = store.acquire_with(&request, &opts).unwrap();
    assert_ne!(first.entry.uuid, second.entry.uuid);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    // Fresh discovery is no substitute for checking cached payload bytes.
    std::fs::write(second.directory.join("index.json"), b"tampered content").unwrap();
    assert!(store.path(second.entry.uuid).is_ok());
    assert!(store.acquire_with(&request, &opts).is_err());
    request.selection.sha256 = Some("0".repeat(64));
    assert!(store.acquire_with(&request, &opts).is_err());
    request.selection.format = Format::Natural;
    assert!(store.acquire_with(&request, &opts).is_err());
    assert_eq!(
        store
            .records()
            .unwrap()
            .iter()
            .filter(|r| matches!(r, Record::Output(_)))
            .count(),
        2
    );
}
