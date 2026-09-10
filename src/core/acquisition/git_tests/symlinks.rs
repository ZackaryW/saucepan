use super::*;
use std::io::Write;

fn link(repo: &Path, name: &str, target: &[u8]) {
    let mut blob = tempfile::NamedTempFile::new().unwrap();
    blob.write_all(target).unwrap();
    let oid = git(
        repo,
        &["hash-object", "-w", "--", blob.path().to_str().unwrap()],
    );
    git(
        repo,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("120000,{oid},{name}"),
        ],
    );
}
fn app(store: &Store) -> AppContext {
    AppContext::authenticated(
        store
            .register(
                "app",
                AppSettings {
                    verify_content: true,
                    ..AppSettings::default()
                },
                Filters::default(),
            )
            .unwrap(),
    )
}

#[test]
fn safe_git_links_export_files_directories_and_cross_folder_content() {
    let dir = tempfile::tempdir().unwrap();
    let remote = dir.path().join("r");
    repository(&remote);
    git(&remote, &["update-index", "--chmod=+x", "b/file"]);
    link(&remote, "a/from-b", b"../b/file");
    link(&remote, "alias-dir", b"b");
    link(&remote, "alias-file", b"alias-dir/file");
    git(&remote, &["commit", "-m", "aliases"]);
    let store = Store::create_test(dir.path().join("s"), [7; 32]).unwrap();
    let app = app(&store);
    let a = store.acquire(&app, &recipe(&remote, "main", "a")).unwrap();
    assert_eq!(fs::read(a.directory.join("from-b")).unwrap(), b"B");
    assert!(a.artifact.files["from-b"].executable);
    let b = store
        .acquire(&app, &recipe(&remote, "main", "alias-dir"))
        .unwrap();
    assert_eq!(a.artifact.snapshot_id, b.artifact.snapshot_id);
    assert_eq!(fs::read(b.directory.join("file")).unwrap(), b"B");
    let state = store
        .source_state(&app, &a.artifact.source_id)
        .unwrap()
        .unwrap();
    let current = state.current.unwrap();
    assert!(current.files["alias-file"].executable);
    let zip = store
        .root
        .join("sources")
        .join(&a.artifact.source_id)
        .join("snapshots")
        .join(format!("{}.zip", current.id));
    let mut archive = zip::ZipArchive::new(fs::File::open(zip).unwrap()).unwrap();
    for i in 0..archive.len() {
        let entry = archive.by_index(i).unwrap();
        assert!(
            !entry
                .name()
                .split('/')
                .any(|p| p.eq_ignore_ascii_case(".git"))
        );
        assert_ne!(entry.unix_mode().unwrap_or_default() & 0o170000, 0o120000);
    }
    let mirror = dir.path().join("mirror");
    store.mirror(&app, &a.artifact.id, &mirror).unwrap();
    fs::write(a.directory.join("from-b"), b"edited").unwrap();
    assert_eq!(fs::read(b.directory.join("file")).unwrap(), b"B");
    assert_eq!(fs::read(mirror.join("from-b")).unwrap(), b"B");
    assert!(store.artifact_path(&app, &a.artifact.id).is_err());
}

#[test]
fn links_cross_exact_submodules_in_both_directions() {
    let dir = tempfile::tempdir().unwrap();
    let remote = dir.path().join("r");
    repository(&remote);
    let child = dir.path().join("d");
    repository(&child);
    link(&child, "a/from-parent", b"../../b/file");
    git(&child, &["commit", "-m", "parent alias"]);
    let oid = git(&child, &["rev-parse", "HEAD"]);
    fs::write(
        remote.join(".gitmodules"),
        format!(
            "[submodule \"dep\"]\npath = dep\nurl = {}\n",
            url::Url::from_file_path(&child).unwrap()
        ),
    )
    .unwrap();
    git(&remote, &["add", ".gitmodules"]);
    git(
        &remote,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{oid},dep"),
        ],
    );
    link(&remote, "a/from-child", b"../dep/a/file");
    git(&remote, &["commit", "-m", "child alias"]);
    let store = Store::create_test(dir.path().join("s"), [7; 32]).unwrap();
    let app = app(&store);
    let a = store.acquire(&app, &recipe(&remote, "main", "a")).unwrap();
    assert_eq!(fs::read(a.directory.join("from-child")).unwrap(), b"A");
    let dep = store
        .acquire(&app, &recipe(&remote, "main", "dep/a"))
        .unwrap();
    assert_eq!(fs::read(dep.directory.join("from-parent")).unwrap(), b"B");
    assert_eq!(
        store
            .source_state(&app, &a.artifact.source_id)
            .unwrap()
            .unwrap()
            .current
            .unwrap()
            .dependencies["dep"],
        oid
    );
    git(
        &remote,
        &[
            "update-index",
            "--cacheinfo",
            &format!("160000,{},dep", "0".repeat(39) + "1"),
        ],
    );
    git(&remote, &["commit", "-m", "missing child"]);
    assert!(store.acquire(&app, &recipe(&remote, "main", "a")).is_err());
}

#[test]
fn git_link_to_lfs_uses_verified_bytes_and_dependency() {
    let dir = tempfile::tempdir().unwrap();
    let remote = dir.path().join("r");
    repository(&remote);
    let bytes = b"actual LFS bytes behind alias";
    let oid = sha256(bytes.as_slice()).unwrap();
    let relative = PathBuf::from("lfs/objects")
        .join(&oid[..2])
        .join(&oid[2..4])
        .join(&oid);
    let object = remote.join(".git").join(&relative);
    fs::create_dir_all(object.parent().unwrap()).unwrap();
    fs::write(object, bytes).unwrap();
    fs::write(
        remote.join("b/file"),
        format!(
            "version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {}\n",
            bytes.len()
        ),
    )
    .unwrap();
    git(&remote, &["add", "b/file"]);
    link(&remote, "a/lfs", b"../b/file");
    git(&remote, &["commit", "-m", "LFS alias"]);
    let store = Store::create_test(dir.path().join("s"), [7; 32]).unwrap();
    let app = app(&store);
    let r = recipe(&remote, "main", "a");
    let acquired = store.acquire(&app, &r).unwrap();
    assert_eq!(fs::read(acquired.directory.join("lfs")).unwrap(), bytes);
    assert_eq!(
        store
            .source_state(&app, &acquired.artifact.source_id)
            .unwrap()
            .unwrap()
            .current
            .unwrap()
            .dependencies["lfs:b/file"],
        oid
    );
    let cached = store
        .root
        .join("sources")
        .join(&acquired.artifact.source_id)
        .join("repo")
        .join(relative);
    fs::write(cached, b"corrupt").unwrap();
    assert!(store.acquire(&app, &r).is_err());
}

#[test]
fn unsafe_git_links_preserve_state_and_never_trigger_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let remote = dir.path().join("r");
    repository(&remote);
    let store = Store::create_test(dir.path().join("s"), [7; 32]).unwrap();
    let app = app(&store);
    let r = recipe(&remote, "main", "a");
    store.acquire(&app, &r).unwrap();
    for (retain_snapshots, verify_content) in [(true, true), (false, false)] {
        store
            .configure(
                &app,
                AppSettings {
                    retain_snapshots,
                    verify_content,
                    allow_local_fallback: true,
                },
                Filters::default(),
            )
            .unwrap();
        for target in [
            b"missing".as_slice(),
            b"../escape",
            b"/host",
            b".git/config",
            b".",
            b"bad",
            &[0xff],
        ] {
            link(&remote, "bad", target);
            git(&remote, &["commit", "-m", "unsafe alias"]);
            let before = fs::read(store.root.join("index.json.enc")).unwrap();
            let view = store.view(&app).unwrap();
            assert!(store.acquire(&app, &r).is_err(), "accepted {target:?}");
            assert_eq!(fs::read(store.root.join("index.json.enc")).unwrap(), before);
            assert_eq!(store.view(&app).unwrap(), view);
        }
    }
    git(&remote, &["update-index", "--force-remove", "bad"]);
    for i in 0..65 {
        let target = if i == 64 {
            "a/file".into()
        } else {
            format!("chain{}", i + 1)
        };
        link(&remote, &format!("chain{i}"), target.as_bytes());
    }
    git(&remote, &["commit", "-m", "excessive hops"]);
    let before = fs::read(store.root.join("index.json.enc")).unwrap();
    assert!(store.acquire(&app, &r).is_err());
    assert_eq!(fs::read(store.root.join("index.json.enc")).unwrap(), before);
}

#[test]
fn git_alias_history_reuses_inputs_rotates_targets_and_reads_exact_pins() {
    let dir = tempfile::tempdir().unwrap();
    let remote = dir.path().join("r");
    repository(&remote);
    let store = Store::create_test(dir.path().join("s"), [7; 32]).unwrap();
    let app = app(&store);
    let r = recipe(&remote, "main", "a");
    let original = store.acquire(&app, &r).unwrap();
    link(&remote, "a/alias", b"../b/file");
    git(&remote, &["commit", "-m", "alias"]);
    let first = store.acquire(&app, &r).unwrap();
    let old = store
        .read_snapshot(
            &app,
            &original.artifact.source_id,
            &original.artifact.snapshot_id,
            Some("a".into()),
        )
        .unwrap();
    assert_eq!(old.artifact, original.artifact);
    assert_eq!(fs::read(old.directory.join("file")).unwrap(), b"A");
    let repeat = store.acquire(&app, &r).unwrap();
    assert_eq!(first.artifact.snapshot_id, repeat.artifact.snapshot_id);
    let mut last = first.clone();
    for n in 0..7 {
        fs::write(remote.join("b/file"), format!("B{n}")).unwrap();
        git(&remote, &["add", "b/file"]);
        git(&remote, &["commit", "-m", "target changed"]);
        last = store.acquire(&app, &r).unwrap();
        assert_eq!(
            fs::read_to_string(last.directory.join("alias")).unwrap(),
            format!("B{n}")
        );
    }
    let history = store
        .source_state(&app, &last.artifact.source_id)
        .unwrap()
        .unwrap();
    assert_eq!(history.history.len(), 5);
    let prior = &history.history[0];
    let read = store
        .read_snapshot(&app, &last.artifact.source_id, &prior.id, Some("a".into()))
        .unwrap();
    assert!(
        !fs::symlink_metadata(read.directory.join("alias"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let mut pin = r.clone();
    pin.commit = Some(first.artifact.revision);
    let pinned = store.acquire(&app, &pin).unwrap();
    assert_eq!(fs::read(pinned.directory.join("alias")).unwrap(), b"B");
    assert_eq!(
        store
            .source_state(&app, &last.artifact.source_id)
            .unwrap()
            .unwrap()
            .current
            .unwrap()
            .id,
        last.artifact.snapshot_id
    );
}
