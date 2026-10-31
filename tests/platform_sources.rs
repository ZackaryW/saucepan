mod native_root;
mod support;
use saucepan::{
    core::source::AcquireOptions,
    error::{Error, ErrorCode},
    models::{
        source::{GitPlatform, Source},
        store::{Request, Selector},
    },
};
use std::{
    io::{Cursor, Write},
    sync::{Arc, Mutex},
};

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

type Log = Arc<Mutex<Vec<(String, String, Option<String>)>>>;

fn header(head: &str, name: &str) -> Option<String> {
    head.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_owned())
    })
}

/// A loopback provider that records method, path and the named auth header.
fn provider(
    auth: &'static str,
    reply: impl Fn(&str, &str) -> (u16, Vec<u8>) + Send + 'static,
) -> (support::Server, Log) {
    let log: Log = Arc::default();
    let seen = log.clone();
    let server = support::Server::with_head(move |head, _| {
        let mut words = head.split_whitespace();
        let method = words.next().unwrap_or("").to_owned();
        let path = words.next().unwrap_or("/").to_owned();
        seen.lock()
            .unwrap()
            .push((method.clone(), path.clone(), header(head, auth)));
        let (status, body) = reply(&path, head);
        support::Reply {
            status,
            body: if method == "HEAD" { Vec::new() } else { body },
            headers: vec![("ETag".into(), "\"v1\"".into())],
        }
    });
    (server, log)
}

fn fake_gh(dir: &std::path::Path) -> std::path::PathBuf {
    #[cfg(windows)]
    {
        let path = dir.join("gh.cmd");
        std::fs::write(&path, "@echo off\r\necho gho_fixture\r\n").unwrap();
        path
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("gh");
        std::fs::write(&path, "#!/bin/sh\necho gho_fixture\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }
}

fn request(source: uuid::Uuid, selector: Selector) -> Request {
    Request {
        source_uuid: source,
        selector,
        selection: Default::default(),
        policy_refs: Default::default(),
    }
}

fn github_options(tmp: &std::path::Path, base: &str) -> AcquireOptions {
    AcquireOptions {
        test_now: Some(100),
        test_github_api: Some(base.into()),
        test_github_raw: Some(base.into()),
        test_gh: Some(fake_gh(tmp)),
        ..Default::default()
    }
}

#[test]
fn github_raw_resolves_branch_to_commit_and_reuses_discovery() {
    let (server, log) = provider("authorization", |path, head| {
        if path == "/repos/owner/repo/commits/main" {
            assert_eq!(
                header(head, "accept").as_deref(),
                Some("application/vnd.github.sha")
            );
            (200, COMMIT.as_bytes().to_vec())
        } else if path == format!("/owner/repo/{COMMIT}/docs/guide.md") {
            (200, b"guide".to_vec())
        } else {
            (404, Vec::new())
        }
    });
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::GithubRaw {
            repo: "Owner/Repo".into(),
            path: "docs/guide.md".into(),
        })
        .unwrap();
    let opts = github_options(tmp.path(), &server.base);
    let main = request(
        source.uuid,
        Selector::Ref {
            reference: "main".into(),
        },
    );
    let result = store.acquire_with(&main, &opts).unwrap();
    assert_eq!(
        std::fs::read(result.directory.join("guide.md")).unwrap(),
        b"guide"
    );
    store.acquire_with(&main, &opts).unwrap();
    let bearer = Some("Bearer gho_fixture".to_owned());
    assert_eq!(
        log.lock().unwrap().clone(),
        vec![
            (
                "GET".into(),
                "/repos/owner/repo/commits/main".into(),
                bearer.clone()
            ),
            (
                "GET".into(),
                format!("/owner/repo/{COMMIT}/docs/guide.md"),
                bearer
            ),
        ],
        "a fresh discovery is reused without another request"
    );
    assert!(
        !tmp.path()
            .join(format!("root/source/{}/repo", source.uuid))
            .exists()
    );
}

#[test]
fn github_raw_commit_selector_skips_resolution() {
    let (server, log) = provider("authorization", |path, _| {
        if path.starts_with(&format!("/owner/repo/{COMMIT}/")) {
            (200, b"pinned".to_vec())
        } else {
            (404, Vec::new())
        }
    });
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::GithubRaw {
            repo: "owner/repo".into(),
            path: "a b.txt".into(),
        })
        .unwrap();
    let result = store
        .acquire_with(
            &request(
                source.uuid,
                Selector::Commit {
                    commit: COMMIT.into(),
                },
            ),
            &github_options(tmp.path(), &server.base),
        )
        .unwrap();
    assert_eq!(
        std::fs::read(result.directory.join("a b.txt")).unwrap(),
        b"pinned"
    );
    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].1, format!("/owner/repo/{COMMIT}/a%20b.txt"));
}

#[test]
fn raw_folder_selection_is_rejected_before_network() {
    let (server, log) = provider("authorization", |_, _| (200, Vec::new()));
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::GithubRaw {
            repo: "owner/repo".into(),
            path: "docs/guide.md".into(),
        })
        .unwrap();
    let mut folder = request(
        source.uuid,
        Selector::Ref {
            reference: "main".into(),
        },
    );
    folder.selection.folder = Some("docs".into());
    let error = store
        .acquire_with(&folder, &github_options(tmp.path(), &server.base))
        .unwrap_err();
    assert_eq!(Error::from_anyhow(&error).code, ErrorCode::InvalidRequest);
    assert!(log.lock().unwrap().is_empty());
}

fn git_raw(platform: GitPlatform, auth: &'static str, route: &'static str, selector: Selector) {
    let expected = route.to_owned();
    let (server, log) = provider(auth, move |path, _| {
        if path == expected {
            (200, b"raw".to_vec())
        } else {
            (404, Vec::new())
        }
    });
    let tmp = tempfile::tempdir().unwrap();
    let token = tmp.path().join("token");
    std::fs::write(&token, "secret-token\n").unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::GitRaw {
            host: "git.example.com".into(),
            platform,
            repo: "group/sub/project".into(),
            path: "dir/file.txt".into(),
            token_file: Some(token.to_str().unwrap().into()),
        })
        .unwrap();
    let opts = AcquireOptions {
        test_now: Some(100),
        test_git_raw: Some(server.base.clone()),
        ..Default::default()
    };
    let result = store
        .acquire_with(&request(source.uuid, selector), &opts)
        .unwrap();
    assert_eq!(
        std::fs::read(result.directory.join("file.txt")).unwrap(),
        b"raw"
    );
    let log = log.lock().unwrap();
    assert!(!log.is_empty());
    for (_, path, sent) in log.iter() {
        assert_eq!(path, route);
        assert!(sent.as_deref().is_some_and(|v| v.contains("secret-token")));
    }
}

#[test]
fn git_raw_uses_gitlab_and_gitea_routes_with_token_file() {
    git_raw(
        GitPlatform::Gitlab,
        "private-token",
        "/group/sub/project/-/raw/main/dir/file.txt",
        Selector::Ref {
            reference: "main".into(),
        },
    );
    git_raw(
        GitPlatform::Gitea,
        "authorization",
        "/group/sub/project/raw/v1/dir/file.txt",
        Selector::Tag { tag: "v1".into() },
    );
}

#[test]
fn unsupported_platform_is_rejected_at_registration() {
    let parsed = serde_json::from_value::<Source>(serde_json::json!({
        "type": "git_raw",
        "host": "bitbucket.org",
        "platform": "bitbucket",
        "repo": "o/r",
        "path": "a"
    }));
    assert!(parsed.is_err());
}

fn snapshot() -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, data) in [
        ("owner-repo-0123456/README.md", &b"readme"[..]),
        ("owner-repo-0123456/skills/a/SKILL.md", b"skill"),
        (
            "owner-repo-0123456/skills/a/big.bin",
            b"version https://git-lfs.github.com/spec/v1\noid sha256:aa\nsize 1\n",
        ),
    ] {
        zip.start_file(name, options).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn github_source_extracts_selected_folder_without_a_repository() {
    let (server, log) = provider("authorization", |path, _| {
        if path == "/repos/owner/repo/commits/main" {
            (200, COMMIT.as_bytes().to_vec())
        } else if path == format!("/repos/owner/repo/zipball/{COMMIT}") {
            (200, snapshot())
        } else {
            (404, Vec::new())
        }
    });
    let tmp = tempfile::tempdir().unwrap();
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::GithubSource {
            repo: "owner/repo".into(),
        })
        .unwrap();
    let mut skills = request(
        source.uuid,
        Selector::Ref {
            reference: "main".into(),
        },
    );
    skills.selection.folder = Some("skills".into());
    let result = store
        .acquire_with(&skills, &github_options(tmp.path(), &server.base))
        .unwrap();
    assert_eq!(
        std::fs::read(result.directory.join("a/SKILL.md")).unwrap(),
        b"skill"
    );
    assert!(
        std::fs::read_to_string(result.directory.join("a/big.bin"))
            .unwrap()
            .starts_with("version https://git-lfs")
    );
    assert!(!result.directory.join("README.md").exists());
    assert!(
        !tmp.path()
            .join(format!("root/source/{}/repo", source.uuid))
            .exists()
    );
    assert_eq!(log.lock().unwrap().len(), 2);
}
