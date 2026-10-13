mod native_root;
use saucepan::{
    core::source::AcquireOptions,
    models::{
        source::{Resolved, Source},
        store::{Request, Selector},
    },
};
use std::{fs, path::Path, process::Command};
fn git(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.test",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().into()
}
#[test]
fn shared_source_tracked_and_pinned_outputs_with_cli_parity() {
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("repo");
    fs::create_dir(&input).unwrap();
    git(&input, &["init", "-b", "main"]);
    fs::create_dir(input.join("folder")).unwrap();
    fs::write(input.join("folder/file"), b"one").unwrap();
    git(&input, &["add", "."]);
    git(&input, &["commit", "-m", "one"]);
    let first = git(&input, &["rev-parse", "HEAD"]);
    let store = native_root::create(&tmp.path().join("root")).unwrap();
    let source = store
        .register_source(Source::Git {
            origin: input.to_str().unwrap().into(),
        })
        .unwrap();
    let alias = store
        .register_source(Source::Git {
            origin: url::Url::from_file_path(&input).unwrap().into(),
        })
        .unwrap();
    assert_eq!(source.uuid, alias.uuid);
    let github = store
        .register_source(Source::GithubRepo {
            repo: "Owner/Project".into(),
        })
        .unwrap();
    assert_eq!(
        github.uuid,
        store
            .register_source(Source::Git {
                origin: "https://github.com/owner/project.git".into()
            })
            .unwrap()
            .uuid
    );
    let mut request = Request {
        adopted: None,
        source_uuid: source.uuid,
        selector: Selector::Ref {
            reference: "main".into(),
        },
        selection: Default::default(),
        policies: Default::default(),
    };
    request.selection.folder = Some("folder".into());
    let mut opts = AcquireOptions {
        test_now: Some(100),
        ..Default::default()
    };
    let a = store.acquire_with(&request, &opts).unwrap();
    fs::write(input.join("folder/file"), b"two").unwrap();
    git(&input, &["add", "."]);
    git(&input, &["commit", "-m", "two"]);
    assert_eq!(
        store.acquire_with(&request, &opts).unwrap().entry.uuid,
        a.entry.uuid
    );
    opts.test_now = Some(86500);
    let b = store.acquire_with(&request, &opts).unwrap();
    assert_ne!(a.entry.uuid, b.entry.uuid);
    assert_eq!(
        a.entry.resolved,
        Resolved::Git {
            commit: first.clone()
        }
    );
    assert_eq!(fs::read(a.directory.join("file")).unwrap(), b"one");
    assert_eq!(fs::read(b.directory.join("file")).unwrap(), b"two");
    request.selector = Selector::Commit { commit: first };
    let file = tmp.path().join("request.json");
    fs::write(&file, serde_json::to_vec(&request).unwrap()).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .arg("--root")
        .arg(store.root())
        .arg("acquire")
        .arg(file)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let acquired: saucepan::models::store::Acquired = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(fs::read(acquired.directory.join("file")).unwrap(), b"one");
    assert!(
        store
            .root()
            .join(format!("source/{}/repo", source.uuid))
            .is_dir()
    );
    request.selector = Selector::Ref {
        reference: "main".into(),
    };
    fs::rename(&input, tmp.path().join("offline")).unwrap();
    opts.force_refresh = true;
    assert!(store.acquire_with(&request, &opts).is_err());
    opts.allow_stale = true;
    let stale = store.acquire_with(&request, &opts).unwrap();
    assert!(stale.stale);
    assert_eq!(stale.entry.uuid, b.entry.uuid);
}
