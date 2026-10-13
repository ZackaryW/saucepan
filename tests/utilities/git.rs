use saucepan::utils::{
    source::git::{Git, RepositoryMaterial},
    tree::TreeLimits,
};
use std::{fs, path::Path, process::Command, time::Duration};

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.test",
            "-c",
            "protocol.file.allow=always",
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
fn init(repo: &Path) {
    fs::create_dir_all(repo).unwrap();
    git(repo, &["init", "-b", "main"]);
}
fn commit(repo: &Path) -> String {
    git(repo, &["add", "."]);
    git(repo, &["commit", "-m", "fixture"]);
    git(repo, &["rev-parse", "HEAD"])
}

#[test]
fn exact_exports_submodules_links_limits_and_failure_cleanup() {
    let tmp = tempfile::tempdir().unwrap();
    let origin = tmp.path().join("origin with spaces");
    let child = tmp.path().join("dependency");
    init(&child);
    fs::write(child.join("dep"), b"dependency").unwrap();
    let child_commit = commit(&child);
    init(&origin);
    fs::create_dir(origin.join("folder")).unwrap();
    fs::write(origin.join("folder/file"), b"first").unwrap();
    git(
        &origin,
        &["submodule", "add", child.to_str().unwrap(), "dep"],
    );
    commit(&origin);
    // Insert a committed symlink without requiring native symlink privileges.
    fs::write(tmp.path().join("link-data"), b"folder/file").unwrap();
    let blob = git(
        &origin,
        &[
            "hash-object",
            "-w",
            tmp.path().join("link-data").to_str().unwrap(),
        ],
    );
    git(
        &origin,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("120000,{blob},alias"),
        ],
    );
    git(&origin, &["update-index", "--chmod=+x", "folder/file"]);
    git(&origin, &["commit", "-m", "links"]);
    let first = git(&origin, &["rev-parse", "HEAD"]);
    let cache = tmp.path().join("cache");
    fs::create_dir(&cache).unwrap();
    let repo = cache.join("repo");
    let utility = Git::default();
    utility.repository(origin.to_str().unwrap(), &repo).unwrap();
    assert_eq!(utility.resolve(&repo, "main", false).unwrap(), first);
    assert!(utility.repository(child.to_str().unwrap(), &repo).is_err());
    let out = tmp.path().join("export");
    let summary = utility
        .export(origin.to_str().unwrap(), &repo, &first, &cache, &out)
        .unwrap();
    assert_eq!(fs::read(out.join("alias")).unwrap(), b"first");
    assert_eq!(fs::read(out.join("dep/dep")).unwrap(), b"dependency");
    assert_eq!(summary.dependencies["dep"], child_commit);
    assert!(summary.executables.contains("alias"));
    assert!(!out.join(".git").exists());
    fs::write(origin.join("folder/file"), b"second").unwrap();
    let second = commit(&origin);
    assert_eq!(utility.resolve(&repo, "main", false).unwrap(), second);
    assert_eq!(utility.resolve(&repo, &first, true).unwrap(), first);
    assert_eq!(fs::read(out.join("folder/file")).unwrap(), b"first");
    assert!(utility.resolve(&repo, "--upload-pack=oops", false).is_err());
    let limited = Git {
        limits: TreeLimits {
            entries: 1,
            bytes: 1,
        },
        ..Git::default()
    };
    let failed = tmp.path().join("failed");
    assert!(
        limited
            .export(origin.to_str().unwrap(), &repo, &first, &cache, &failed)
            .is_err()
    );
    assert!(!failed.exists());
    let timeout = Git {
        timeout: Duration::ZERO,
        ..Git::default()
    };
    assert!(timeout.resolve(&repo, "main", false).is_err());
    assert_eq!(fs::read(out.join("alias")).unwrap(), b"first");
}

#[test]
fn lfs_bytes_are_resolved_and_verified() {
    let tmp = tempfile::tempdir().unwrap();
    let origin = tmp.path().join("origin");
    init(&origin);
    git(&origin, &["lfs", "install", "--local"]);
    git(&origin, &["lfs", "track", "*.dat"]);
    fs::write(origin.join("payload.dat"), b"real LFS content").unwrap();
    let revision = commit(&origin);
    let cache = tmp.path().join("cache");
    fs::create_dir(&cache).unwrap();
    let repo = cache.join("repo");
    let utility = Git::default();
    utility.repository(origin.to_str().unwrap(), &repo).unwrap();
    let out = tmp.path().join("out");
    let result = utility
        .export(origin.to_str().unwrap(), &repo, &revision, &cache, &out)
        .unwrap();
    assert_eq!(
        fs::read(out.join("payload.dat")).unwrap(),
        b"real LFS content"
    );
    assert!(result.dependencies.contains_key("lfs:payload.dat"));
    // Tampering with the cached LFS object must not publish another output.
    let oid = &result.dependencies["lfs:payload.dat"];
    fs::write(
        repo.join("lfs/objects")
            .join(&oid[..2])
            .join(&oid[2..4])
            .join(oid),
        b"bad",
    )
    .unwrap();
    let failed = tmp.path().join("bad");
    assert!(
        utility
            .export(origin.to_str().unwrap(), &repo, &revision, &cache, &failed)
            .is_err()
    );
    assert!(!failed.exists());
}

#[test]
fn empty_export_and_broken_committed_link() {
    let tmp = tempfile::tempdir().unwrap();
    let origin = tmp.path().join("origin");
    init(&origin);
    git(&origin, &["commit", "--allow-empty", "-m", "empty"]);
    let empty = git(&origin, &["rev-parse", "HEAD"]);
    let cache = tmp.path().join("cache");
    fs::create_dir(&cache).unwrap();
    let repo = cache.join("repo");
    let utility = Git::default();
    utility.repository(origin.to_str().unwrap(), &repo).unwrap();
    let out = tmp.path().join("empty");
    utility
        .export(origin.to_str().unwrap(), &repo, &empty, &cache, &out)
        .unwrap();
    assert!(fs::read_dir(out).unwrap().next().is_none());
    fs::write(tmp.path().join("link"), b"../outside").unwrap();
    let blob = git(
        &origin,
        &[
            "hash-object",
            "-w",
            tmp.path().join("link").to_str().unwrap(),
        ],
    );
    git(
        &origin,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("120000,{blob},bad"),
        ],
    );
    git(&origin, &["commit", "-m", "bad link"]);
    let bad = utility.resolve(&repo, "main", false).unwrap();
    let out = tmp.path().join("bad");
    assert!(
        utility
            .export(origin.to_str().unwrap(), &repo, &bad, &cache, &out)
            .is_err()
    );
    assert!(!out.exists());
}

#[test]
fn offline_export_requires_hydrated_dependencies_and_never_repairs_them() {
    let tmp = tempfile::tempdir().unwrap();
    let origin = tmp.path().join("origin");
    let child = tmp.path().join("dependency");
    init(&child);
    fs::write(child.join("child"), b"dependency").unwrap();
    commit(&child);
    init(&origin);
    git(&origin, &["lfs", "install", "--local"]);
    git(&origin, &["lfs", "track", "*.dat"]);
    fs::write(origin.join("payload.dat"), b"LFS dependency").unwrap();
    git(
        &origin,
        &["submodule", "add", child.to_str().unwrap(), "dep"],
    );
    let revision = commit(&origin);
    let cache = tmp.path().join("cache");
    fs::create_dir(&cache).unwrap();
    let repo = cache.join("repo");
    let utility = Git::default();
    utility.repository(origin.to_str().unwrap(), &repo).unwrap();
    let origin_url = saucepan::utils::source::git::normalize(origin.to_str().unwrap()).unwrap();
    let material = RepositoryMaterial {
        origin: &origin_url,
        repository: &repo,
        commit: &revision,
        dependencies: &cache,
    };
    let failed = tmp.path().join("before-hydration");
    assert!(
        utility
            .export_offline(material, tmp.path(), &failed)
            .is_err()
    );
    assert!(!cache.join("submodules").exists());
    assert!(!failed.exists());
    let hydrated = utility.hydrate(material, tmp.path()).unwrap();
    let output = tmp.path().join("offline");
    // Both remotes disappear. Every dependency needed by export must already exist.
    fs::rename(&origin, tmp.path().join("offline-origin")).unwrap();
    fs::rename(&child, tmp.path().join("offline-child")).unwrap();
    let exported = utility
        .export_offline(material, tmp.path(), &output)
        .unwrap();
    assert_eq!(exported.dependencies, hydrated.dependencies);
    assert_eq!(
        fs::read(output.join("payload.dat")).unwrap(),
        b"LFS dependency"
    );
    assert_eq!(fs::read(output.join("dep/child")).unwrap(), b"dependency");
    let oid = &hydrated.dependencies["lfs:payload.dat"];
    let object = repo
        .join("lfs/objects")
        .join(&oid[..2])
        .join(&oid[2..4])
        .join(oid);
    fs::remove_file(&object).unwrap();
    // Restore reachable remotes; offline export must still refuse missing material.
    fs::rename(tmp.path().join("offline-origin"), &origin).unwrap();
    fs::rename(tmp.path().join("offline-child"), &child).unwrap();
    assert!(
        utility
            .export_offline(material, tmp.path(), &tmp.path().join("missing-lfs"))
            .is_err()
    );
    assert!(!object.exists());
}
