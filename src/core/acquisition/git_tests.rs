use super::*;
use std::process::Command;
mod symlinks;

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Saucepan tests",
            "-c",
            "user.email=saucepan@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
fn repository(path: &Path) {
    fs::create_dir(path).unwrap();
    git(path, &["init", "-b", "main"]);
    fs::create_dir_all(path.join("a")).unwrap();
    fs::create_dir_all(path.join("b")).unwrap();
    fs::write(path.join("a/file"), b"A").unwrap();
    fs::write(path.join("b/file"), b"B").unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-m", "initial"]);
}
fn recipe(path: &Path, reference: &str, folder: &str) -> Recipe {
    Recipe {
        source: Source::Git {
            origin: url::Url::from_file_path(path).unwrap().to_string(),
            reference: reference.into(),
        },
        folder: Some(folder.into()),
        commit: None,
    }
}

#[test]
fn git_refs_share_folder_exports_track_updates_and_pin_exact_commits() {
    let dir = tempfile::tempdir().unwrap();
    let remote = dir.path().join("remote");
    repository(&remote);
    git(&remote, &["branch", "develop"]);
    let store = Store::create_test(dir.path().join("store"), [7; 32]).unwrap();
    let app = AppContext::authenticated(
        store
            .register("app", AppSettings::default(), Filters::default())
            .unwrap(),
    );
    let a = store.acquire(&app, &recipe(&remote, "main", "a")).unwrap();
    let b = store.acquire(&app, &recipe(&remote, "main", "b")).unwrap();
    let develop = store
        .acquire(&app, &recipe(&remote, "develop", "a"))
        .unwrap();
    assert_eq!(a.artifact.snapshot_id, b.artifact.snapshot_id);
    assert_ne!(a.artifact.source_id, develop.artifact.source_id);
    assert_eq!(fs::read(a.directory.join("file")).unwrap(), b"A");
    fs::write(remote.join("a/file"), b"new").unwrap();
    git(&remote, &["commit", "-am", "advance"]);
    let newer = store.acquire(&app, &recipe(&remote, "main", "a")).unwrap();
    assert_eq!(newer.artifact.source_id, a.artifact.source_id);
    assert_ne!(newer.artifact.revision, a.artifact.revision);
    let mut pin = recipe(&remote, "main", "a");
    pin.commit = Some(a.artifact.revision.clone());
    let pinned = store.acquire(&app, &pin).unwrap();
    assert_eq!(pinned.artifact.revision, a.artifact.revision);
    assert_eq!(fs::read(pinned.directory.join("file")).unwrap(), b"A");
    let pinned_state = store
        .source_state(&app, &a.artifact.source_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        pinned_state
            .history
            .iter()
            .find(|s| s.id == a.artifact.snapshot_id)
            .unwrap()
            .last_used,
        pinned_state.sequence,
        "an exact acquisition must refresh historical LRU recency"
    );
    assert_eq!(
        store
            .source_state(&app, &a.artifact.source_id)
            .unwrap()
            .unwrap()
            .current
            .unwrap()
            .revision,
        newer.artifact.revision
    );
    assert_eq!(
        store
            .source_state(&app, &develop.artifact.source_id)
            .unwrap()
            .unwrap()
            .current
            .unwrap()
            .revision,
        develop.artifact.revision
    );
    let zip = store
        .root
        .join("sources")
        .join(&a.artifact.source_id)
        .join("snapshots")
        .join(format!("{}.zip", a.artifact.snapshot_id));
    let archive = zip::ZipArchive::new(fs::File::open(zip).unwrap()).unwrap();
    assert!(
        !archive
            .file_names()
            .any(|path| path.split('/').any(|part| part == ".git"))
    );
}

#[test]
fn repointed_git_origin_fails_on_acquisition_views_and_content_reads() {
    let dir = tempfile::tempdir().unwrap();
    let remote = dir.path().join("remote");
    repository(&remote);
    let store = Store::create_test(dir.path().join("store"), [7; 32]).unwrap();
    let app = AppContext::authenticated(
        store
            .register(
                "app",
                AppSettings {
                    allow_local_fallback: true,
                    ..AppSettings::default()
                },
                Filters::default(),
            )
            .unwrap(),
    );
    let recipe = recipe(&remote, "main", "a");
    let result = store.acquire(&app, &recipe).unwrap();
    let repo = store
        .root
        .join("sources")
        .join(&result.artifact.source_id)
        .join("repo");
    git(
        &repo,
        &[
            "remote",
            "set-url",
            "origin",
            "https://example.invalid/other",
        ],
    );
    assert!(store.acquire(&app, &recipe).is_err());
    assert!(store.artifact_path(&app, &result.artifact.id).is_err());
    assert!(store.view(&app).is_err());
}

#[test]
fn git_exports_exact_submodules_and_fails_for_missing_required_content() {
    let dir = tempfile::tempdir().unwrap();
    let remote = dir.path().join("remote");
    let dependency = dir.path().join("dependency");
    repository(&remote);
    repository(&dependency);
    let commit = git(&dependency, &["rev-parse", "HEAD"]);
    let url = url::Url::from_file_path(&dependency).unwrap();
    fs::write(
        remote.join(".gitmodules"),
        format!("[submodule \"dep\"]\npath = dep\nurl = {url}\n"),
    )
    .unwrap();
    git(&remote, &["add", ".gitmodules"]);
    git(
        &remote,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{commit},dep"),
        ],
    );
    git(&remote, &["commit", "-m", "submodule"]);
    let store = Store::create_test(dir.path().join("store"), [7; 32]).unwrap();
    let app = AppContext::ordinary("app");
    store
        .register("app", AppSettings::default(), Filters::default())
        .unwrap();
    let result = store
        .acquire(&app, &recipe(&remote, "main", "dep/a"))
        .unwrap();
    assert_eq!(fs::read(result.directory.join("file")).unwrap(), b"A");
    let state = store
        .source_state(&app, &result.artifact.source_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        state.current.unwrap().dependencies.get("dep"),
        Some(&commit)
    );
    git(
        &remote,
        &[
            "update-index",
            "--cacheinfo",
            &format!("160000,{},dep", "0".repeat(39) + "1"),
        ],
    );
    git(&remote, &["commit", "-m", "missing dependency"]);
    assert!(
        store
            .acquire(&app, &recipe(&remote, "main", "dep/a"))
            .is_err()
    );
}

#[test]
fn git_lfs_exports_verified_object_bytes_and_rejects_corrupt_cache() {
    let dir = tempfile::tempdir().unwrap();
    let remote = dir.path().join("remote");
    repository(&remote);
    let data = b"actual LFS object bytes";
    let oid = sha256(data.as_slice()).unwrap();
    let object_relative = PathBuf::from("lfs/objects")
        .join(&oid[..2])
        .join(&oid[2..4])
        .join(&oid);
    let remote_object = remote.join(".git").join(&object_relative);
    fs::create_dir_all(remote_object.parent().unwrap()).unwrap();
    fs::write(&remote_object, data).unwrap();
    fs::write(
        remote.join(".gitattributes"),
        "a/file filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    fs::write(
        remote.join("a/file"),
        format!(
            "version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {}\n",
            data.len()
        ),
    )
    .unwrap();
    git(&remote, &["add", "."]);
    git(&remote, &["commit", "-m", "LFS object"]);
    let store = Store::create_test(dir.path().join("store"), [7; 32]).unwrap();
    store
        .register("app", AppSettings::default(), Filters::default())
        .unwrap();
    let app = AppContext::ordinary("app");
    let recipe = recipe(&remote, "main", "a");
    let result = store.acquire(&app, &recipe).unwrap();
    assert_eq!(fs::read(result.directory.join("file")).unwrap(), data);
    let state = store
        .source_state(&app, &result.artifact.source_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        state.current.unwrap().dependencies.get("lfs:a/file"),
        Some(&oid)
    );
    let cached = store
        .root
        .join("sources")
        .join(&result.artifact.source_id)
        .join("repo")
        .join(object_relative);
    fs::write(cached, b"corrupt").unwrap();
    assert!(store.acquire(&app, &recipe).is_err());
}

#[test]
fn git_refresh_failure_falls_back_only_when_enabled_and_never_substitutes_a_pin() {
    let dir = tempfile::tempdir().unwrap();
    let remote = dir.path().join("remote");
    repository(&remote);
    let store = Store::create_test(dir.path().join("store"), [7; 32]).unwrap();
    store
        .register("app", AppSettings::default(), Filters::default())
        .unwrap();
    let app = AppContext::ordinary("app");
    let mut recipe = recipe(&remote, "main", "a");
    let first = store.acquire(&app, &recipe).unwrap();
    fs::rename(&remote, dir.path().join("offline")).unwrap();
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
    let fallback = store.acquire(&app, &recipe).unwrap();
    assert!(fallback.fallback);
    assert_eq!(fallback.artifact.revision, first.artifact.revision);
    recipe.commit = Some("f".repeat(40));
    assert!(store.acquire(&app, &recipe).is_err());
    recipe.commit = Some(first.artifact.revision.clone());
    let exact = store.acquire(&app, &recipe).unwrap();
    assert_eq!(exact.artifact.revision, first.artifact.revision);
    assert!(!exact.update_checked);
}

#[test]
fn git_executable_metadata_is_preserved_even_on_windows() {
    let dir = tempfile::tempdir().unwrap();
    let remote = dir.path().join("remote");
    repository(&remote);
    git(&remote, &["update-index", "--chmod=+x", "a/file"]);
    git(&remote, &["commit", "-m", "executable"]);
    let store = Store::create_test(dir.path().join("store"), [7; 32]).unwrap();
    store
        .register(
            "app",
            AppSettings {
                verify_content: true,
                ..AppSettings::default()
            },
            Filters::default(),
        )
        .unwrap();
    let app = AppContext::ordinary("app");
    let acquired = store.acquire(&app, &recipe(&remote, "main", "a")).unwrap();
    assert!(acquired.artifact.files["file"].executable);
    let zip = store
        .root
        .join("sources")
        .join(&acquired.artifact.source_id)
        .join("snapshots")
        .join(format!("{}.zip", acquired.artifact.snapshot_id));
    let mut archive = zip::ZipArchive::new(fs::File::open(zip).unwrap()).unwrap();
    assert_ne!(
        archive.by_name("a/file").unwrap().unix_mode().unwrap() & 0o111,
        0
    );
    store.artifact_path(&app, &acquired.artifact.id).unwrap();
}

#[cfg(windows)]
#[test]
fn git_directory_case_collisions_fail_before_publication() {
    let dir = tempfile::tempdir().unwrap();
    let remote = dir.path().join("remote");
    repository(&remote);
    let blob = git(&remote, &["rev-parse", "HEAD:a/file"]);
    git(
        &remote,
        &[
            "-c",
            "core.ignorecase=false",
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("100644,{blob},A/other"),
        ],
    );
    git(&remote, &["commit", "-m", "case collision"]);
    let store = Store::create_test(dir.path().join("store"), [7; 32]).unwrap();
    store
        .register("app", AppSettings::default(), Filters::default())
        .unwrap();
    assert!(
        store
            .acquire(&AppContext::ordinary("app"), &recipe(&remote, "main", "a"))
            .is_err()
    );
}
