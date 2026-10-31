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
        if path.starts_with("/repos/") && !path.contains("/releases/assets/") {
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
        b"/repos/owner/tool/releases/assets/1"
    );
    fail.store(true, Ordering::SeqCst);
    policy_fixture::replace(
        &store,
        &mut request.policy_refs,
        serde_json::json!({"type":"refresh_policy","mode":"always"}),
    )
    .unwrap();
    assert!(store.acquire_with(&request, &opts).is_err());
    policy_fixture::replace(
        &store,
        &mut request.policy_refs,
        serde_json::json!({"type":"stale_policy","allow_stale":true}),
    )
    .unwrap();
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
    policy_fixture::replace(
        &store,
        &mut request.policy_refs,
        serde_json::json!({"type":"refresh_policy","mode":"max_age","seconds":86400}),
    )
    .unwrap();
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
        if path.starts_with("/repos/") && !path.contains("/releases/assets/") {
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

fn fake_gh(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
    #[cfg(windows)]
    {
        let path = dir.join("gh.cmd");
        std::fs::write(&path, format!("@echo off\r\n{body}\r\n")).unwrap();
        path
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("gh");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }
}

fn header(head: &str, name: &str) -> Option<String> {
    head.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_owned())
    })
}

fn release_request(store: &saucepan::core::Store, source: uuid::Uuid) -> Request {
    let mut request = Request {
        source_uuid: source,
        selector: Selector::LatestStable,
        selection: Default::default(),
        policy_refs: Default::default(),
    };
    request.selection.format = Format::Raw;
    request.selection.filter_refs = policy_fixture::filters(
        store,
        vec![AssetFilter {
            platform: None,
            asset_patterns: vec!["*.bin".into()],
        }],
    );
    request
}

#[test]
fn release_requests_carry_gh_token_and_redirects_drop_it() {
    use std::sync::Mutex;
    let seen = Arc::new(Mutex::new(Vec::<(String, Option<String>)>::new()));
    let log = seen.clone();
    let server = support::Server::with_head(move |head, base| {
        let path = head.split_whitespace().nth(1).unwrap_or("/").to_owned();
        log.lock()
            .unwrap()
            .push((path.clone(), header(head, "authorization")));
        if path.ends_with("/releases/assets/7") {
            assert_eq!(
                header(head, "accept").as_deref(),
                Some("application/octet-stream")
            );
            support::Reply {
                status: 302,
                body: Vec::new(),
                headers: vec![("Location".into(), format!("{base}/download/7"))],
            }
        } else if path == "/download/7" {
            support::Reply {
                status: 200,
                body: b"payload".to_vec(),
                headers: Vec::new(),
            }
        } else {
            support::Reply {
                status: 200,
                body: serde_json::json!({"id":1,"tag_name":"v1","draft":false,"prerelease":false,"assets":[{"id":7,"name":"tool.bin","browser_download_url":format!("{base}/browser/7"),"digest":null}]}).to_string().into_bytes(),
                headers: Vec::new(),
            }
        }
    });
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::GithubRelease {
            repo: "owner/private".into(),
        })
        .unwrap();
    let opts = AcquireOptions {
        test_now: Some(100),
        test_github_api: Some(server.base.clone()),
        test_gh: Some(fake_gh(tmp.path(), "echo gho_fixture")),
        ..Default::default()
    };
    let result = store
        .acquire_with(&release_request(&store, source.uuid), &opts)
        .unwrap();
    assert_eq!(
        std::fs::read(result.directory.join("tool.bin")).unwrap(),
        b"payload"
    );
    let seen = seen.lock().unwrap().clone();
    let bearer = Some("Bearer gho_fixture".to_owned());
    assert_eq!(
        seen,
        vec![
            (
                "/repos/owner/private/releases/latest".into(),
                bearer.clone()
            ),
            ("/repos/owner/private/releases/assets/7".into(), bearer),
            ("/download/7".into(), None),
        ]
    );
}

#[test]
fn denied_release_reports_provider_status_without_token_text() {
    use saucepan::error::{Error, ErrorCode};
    let status = Arc::new(AtomicUsize::new(401));
    let s = status.clone();
    let server = support::Server::new(move |_, _| (s.load(Ordering::SeqCst) as u16, Vec::new()));
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::GithubRelease {
            repo: "owner/private".into(),
        })
        .unwrap();
    let request = release_request(&store, source.uuid);
    let mut opts = AcquireOptions {
        test_now: Some(100),
        test_github_api: Some(server.base.clone()),
        test_gh: Some(fake_gh(tmp.path(), "echo gho_fixture")),
        ..Default::default()
    };
    let denied = store.acquire_with(&request, &opts).unwrap_err();
    let error = Error::from_anyhow(&denied);
    assert_eq!(error.code, ErrorCode::SourceError);
    assert_eq!(error.details["provider"], "github");
    assert_eq!(error.details["http_status"], "401");
    assert_eq!(error.details["token_sent"], "true");
    assert!(error.message.contains("gh auth login"));
    assert!(!format!("{denied:?}").contains("gho_fixture"));

    status.store(404, Ordering::SeqCst);
    opts.test_gh = Some(tmp.path().join("missing-gh"));
    let error = Error::from_anyhow(&store.acquire_with(&request, &opts).unwrap_err());
    assert_eq!(error.code, ErrorCode::SourceError);
    assert_eq!(error.details["token_sent"], "false");
    assert!(error.message.contains("gh auth login"));
}

#[test]
fn denied_refresh_preserves_prior_output() {
    let denied = Arc::new(AtomicBool::new(false));
    let d = denied.clone();
    let server = support::Server::new(move |path, base| {
        if d.load(Ordering::SeqCst) {
            (401, Vec::new())
        } else if path.contains("/releases/assets/") {
            (200, b"first".to_vec())
        } else {
            (200, serde_json::json!({"id":1,"tag_name":"v1","draft":false,"prerelease":false,"assets":[{"id":1,"name":"tool.bin","browser_download_url":format!("{base}/x"),"digest":null}]}).to_string().into_bytes())
        }
    });
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::GithubRelease {
            repo: "owner/tool".into(),
        })
        .unwrap();
    let request = release_request(&store, source.uuid);
    let mut opts = AcquireOptions {
        test_now: Some(100),
        test_github_api: Some(server.base.clone()),
        ..Default::default()
    };
    let first = store.acquire_with(&request, &opts).unwrap();
    denied.store(true, Ordering::SeqCst);
    opts.test_now = Some(86500);
    let error =
        saucepan::error::Error::from_anyhow(&store.acquire_with(&request, &opts).unwrap_err());
    assert_eq!(error.details["http_status"], "401");
    assert_eq!(
        std::fs::read(store.path(first.entry.uuid).unwrap().join("tool.bin")).unwrap(),
        b"first"
    );
    let discovery = &store.source(source.uuid).unwrap().discovery;
    assert!(discovery.values().all(|entry| entry.checked_at == 100));
}
