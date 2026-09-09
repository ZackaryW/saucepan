use super::*;
use crate::core::{acquisition, sources};
use std::{collections::BTreeMap, io::Read, process::Command};

#[test]
fn submodule_links_are_checked_against_the_complete_selected_artifact() {
    let f = Fixture::new();
    let child = f._temp.path().join("linked-child");
    std::fs::create_dir(&child).unwrap();
    git(&child, &["init", "--initial-branch=main", "--template="]);
    // Write a committed symlink through Git's index so this fixture does not
    // depend on Windows symlink privileges or a materialized child checkout.
    std::fs::write(child.join("link-value"), "../version.txt").unwrap();
    let blob = git(&child, &["hash-object", "-w", "link-value"]);
    git(
        &child,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("120000,{blob},parent-version"),
        ],
    );
    git(&child, &["commit", "-m", "relative link into parent"]);
    let commit = git(&child, &["rev-parse", "HEAD"]);
    std::fs::write(
        f.repo.join(".gitmodules"),
        "[submodule \"child\"]\npath = pkg/dependency\nurl = ../linked-child\n",
    )
    .unwrap();
    git(&f.repo, &["add", ".gitmodules"]);
    git(
        &f.repo,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{commit},pkg/dependency"),
        ],
    );
    git(&f.repo, &["commit", "-m", "include child"]);
    let identity = sources::identify(&Recipe::git(child.to_str().unwrap()).source, &f.app).unwrap();
    let owner: Marker =
        serde_json::from_slice(&std::fs::read(f.root.join("owner.saucepanhash")).unwrap()).unwrap();
    let context = f.store.inspect_context(&f.app, None).unwrap();
    let mut grants = context.grants.clone();
    grants.push(Grant {
        source_id: Some(identity.id),
        selection: ".".into(),
        actions: [Action::Setup, Action::Inspect].into_iter().collect(),
        destinations: vec![],
    });
    f.store
        .update_application(
            &f.root,
            Some(&owner),
            &context.id,
            Some(ApplicationRegistration {
                root: context.root,
                mode: context.mode,
                grants,
                require_verification: false,
            }),
        )
        .unwrap();
    let artifact = f.acquire().unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(
        std::fs::read(f.archive(&artifact.id)).unwrap(),
    ))
    .unwrap();
    let mut target = String::new();
    let mut link = zip.by_name("dependency/parent-version").unwrap();
    assert_eq!(link.unix_mode().unwrap() & 0o170000, 0o120000);
    link.read_to_string(&mut target).unwrap();
    assert_eq!(target, "../version.txt");
    let before = serde_json::to_value(f.source()).unwrap();
    let mut narrower = f.recipe.clone();
    narrower.export.subdirectory = "pkg/dependency".into();
    assert!(
        matches!(acquisition::acquire(&f.store, &f.app, None, &narrower, &VerificationRequest { content: Some(false) }), Err(e) if e.kind == ErrorKind::Integrity)
    );
    assert_eq!(serde_json::to_value(f.source()).unwrap(), before);
}

#[test]
fn ignored_submodules_need_no_dependency_access_and_missing_required_metadata_fails() {
    let f = Fixture::new();
    let commit = git(&f.repo, &["rev-parse", "HEAD"]);
    std::fs::write(
        f.repo.join(".gitattributes"),
        "pkg/dependency export-ignore\n",
    )
    .unwrap();
    git(&f.repo, &["add", ".gitattributes"]);
    git(
        &f.repo,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{commit},pkg/dependency"),
        ],
    );
    git(
        &f.repo,
        &["commit", "-m", "excluded dependency without metadata"],
    );
    let acquired = f.acquire().unwrap();
    assert!(acquired.inputs.dependencies.is_empty());
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(
        std::fs::read(f.archive(&acquired.id)).unwrap(),
    ))
    .unwrap();
    assert!(zip.by_name("dependency/").is_err());
    let before = serde_json::to_value(f.source()).unwrap();
    std::fs::write(f.repo.join(".gitattributes"), "").unwrap();
    git(&f.repo, &["add", ".gitattributes"]);
    git(
        &f.repo,
        &["commit", "-m", "required dependency without metadata"],
    );
    assert!(matches!(f.acquire(), Err(e) if e.kind == ErrorKind::Source));
    assert_eq!(serde_json::to_value(f.source()).unwrap(), before);
}

#[test]
fn nested_submodules_use_exact_commits_independent_grants_and_cross_boundary_selections() {
    let f = Fixture::new();
    let initial = f.acquire().unwrap();
    let child = f._temp.path().join("child");
    let nested = f._temp.path().join("nested");
    for path in [&child, &nested] {
        std::fs::create_dir(path).unwrap();
        git(path, &["init", "--initial-branch=main", "--template="]);
    }
    std::fs::write(nested.join("nested.txt"), "nested pinned data").unwrap();
    let lfs_bytes = b"nested LFS payload";
    let lfs_oid = crate::utils::hex(&Sha256::digest(lfs_bytes));
    let lfs_object = nested
        .join(".git/lfs/objects")
        .join(&lfs_oid[..2])
        .join(&lfs_oid[2..4])
        .join(&lfs_oid);
    std::fs::create_dir_all(lfs_object.parent().unwrap()).unwrap();
    std::fs::write(&lfs_object, lfs_bytes).unwrap();
    std::fs::write(
        nested.join("large.bin"),
        format!(
            "version https://git-lfs.github.com/spec/v1\noid sha256:{lfs_oid}\nsize {}\n",
            lfs_bytes.len()
        ),
    )
    .unwrap();
    git(&nested, &["add", "."]);
    git(&nested, &["commit", "-m", "nested"]);
    let nested_commit = git(&nested, &["rev-parse", "HEAD"]);
    std::fs::create_dir(child.join("lib")).unwrap();
    std::fs::write(child.join("lib/child.txt"), "child pinned data").unwrap();
    std::fs::write(child.join("sibling.txt"), "exclude when selecting lib").unwrap();
    std::fs::write(
        child.join(".gitmodules"),
        "[submodule \"nested\"]\npath = lib/nested\nurl = ../nested\nupdate = none\n",
    )
    .unwrap();
    git(&child, &["add", "."]);
    git(
        &child,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{nested_commit},lib/nested"),
        ],
    );
    git(&child, &["commit", "-m", "child"]);
    let child_commit = git(&child, &["rev-parse", "HEAD"]);
    std::fs::write(
        f.repo.join(".gitmodules"),
        "[submodule \"child\"]\npath = pkg/dependency\nurl = ../child\n",
    )
    .unwrap();
    git(&f.repo, &["add", ".gitmodules"]);
    git(
        &f.repo,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{child_commit},pkg/dependency"),
        ],
    );
    git(&f.repo, &["commit", "-m", "parent with dependency"]);
    let child_id = sources::identify(&Recipe::git(child.to_str().unwrap()).source, &f.app).unwrap();
    let nested_id =
        sources::identify(&Recipe::git(nested.to_str().unwrap()).source, &f.app).unwrap();
    assert!(matches!(f.acquire(), Err(e) if e.kind == ErrorKind::Authority));
    assert_eq!(
        f.source().current[&initial.inputs.stream_id].artifact.id,
        initial.id
    );
    assert!(!f.root.join("sources").join(&child_id.id).exists());
    let owner: Marker =
        serde_json::from_slice(&std::fs::read(f.root.join("owner.saucepanhash")).unwrap()).unwrap();
    let context = f.store.inspect_context(&f.app, None).unwrap();
    let mut grants = context.grants.clone();
    for identity in [&child_id, &nested_id] {
        grants.push(Grant {
            source_id: Some(identity.id.clone()),
            selection: ".".into(),
            actions: [Action::Setup, Action::Inspect, Action::Remove]
                .into_iter()
                .collect(),
            destinations: vec![],
        });
        f.store
            .update_application(
                &f.root,
                Some(&owner),
                &context.id,
                Some(ApplicationRegistration {
                    root: context.root.clone(),
                    mode: context.mode,
                    grants: grants.clone(),
                    require_verification: false,
                }),
            )
            .unwrap();
        if identity.id == child_id.id {
            assert!(matches!(f.acquire(), Err(e) if e.kind == ErrorKind::Authority));
            assert!(!f.root.join("sources").join(&nested_id.id).exists());
        }
    }
    let acquired = f.acquire().unwrap();
    assert_eq!(acquired.inputs.dependencies.len(), 2);
    assert_eq!(acquired.inputs.dependencies[0].commit, child_commit);
    assert_eq!(acquired.inputs.dependencies[1].commit, nested_commit);
    assert_eq!(
        acquired.inputs.lfs_objects[0].path,
        "dependency/lib/nested/large.bin"
    );
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(
        std::fs::read(f.archive(&acquired.id)).unwrap(),
    ))
    .unwrap();
    assert!(zip.by_name("dependency/lib/child.txt").is_ok());
    assert!(zip.by_name("dependency/lib/nested/nested.txt").is_ok());
    assert!(zip.file_names().all(|name| {
        !name
            .split('/')
            .any(|part| part.eq_ignore_ascii_case(".git"))
    }));
    std::fs::write(child.join("lib/child.txt"), "new unpinned child").unwrap();
    git(&child, &["commit", "-am", "advance child only"]);
    assert_eq!(f.acquire().unwrap().id, acquired.id);
    let mut selection = f.recipe.clone();
    selection.export.subdirectory = "pkg/dependency/lib".into();
    let selected = acquisition::acquire(
        &f.store,
        &f.app,
        None,
        &selection,
        &VerificationRequest { content: None },
    )
    .unwrap();
    assert_eq!(selected.inputs.dependencies[0].path, ".");
    assert_eq!(selected.inputs.dependencies[0].subdirectory, "lib");
    let materialized = f
        .store
        .materialize(
            &f.app,
            None,
            &selection,
            &selected.id,
            &VerificationRequest { content: None },
        )
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(Path::new(&materialized.path).join("child.txt")).unwrap(),
        "child pinned data"
    );
    assert!(
        Path::new(&materialized.path)
            .join("nested/nested.txt")
            .is_file()
    );
    assert!(!Path::new(&materialized.path).join("sibling.txt").exists());
    assert_eq!(
        std::fs::read(Path::new(&materialized.path).join("nested/large.bin")).unwrap(),
        lfs_bytes
    );
    std::fs::remove_file(&lfs_object).unwrap();
    assert_eq!(f.acquire().unwrap().id, acquired.id);
    let cached_child = f.root.join("sources").join(&child_id.id).join("repo.git");
    assert!(
        !git(
            &cached_child,
            &[
                "for-each-ref",
                "--format=%(refname)",
                "refs/saucepan/dependencies/"
            ]
        )
        .is_empty()
    );
    git(
        &cached_child,
        &["remote", "set-url", "origin", nested.to_str().unwrap()],
    );
    assert!(matches!(f.acquire(), Err(e) if e.kind == ErrorKind::Integrity));
    assert!(
        matches!(f.store.materialization_path(&f.app, None, &materialized.id, &VerificationRequest { content: Some(false) }), Err(e) if e.kind == ErrorKind::Integrity)
    );
    git(
        &cached_child,
        &["remote", "set-url", "origin", &child_id.origin],
    );
    grants.retain(|grant| grant.source_id.as_deref() != Some(&nested_id.id));
    f.store
        .update_application(
            &f.root,
            Some(&owner),
            &context.id,
            Some(ApplicationRegistration {
                root: context.root,
                mode: context.mode,
                grants,
                require_verification: false,
            }),
        )
        .unwrap();
    assert!(
        matches!(f.store.materialization_path(&f.app, None, &materialized.id, &VerificationRequest { content: None }), Err(e) if e.kind == ErrorKind::NotFound)
    );
}

#[test]
fn lfs_provenance_reuse_and_failed_refresh_preserve_selected_current() {
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let content = b"downloaded LFS artifact\n";
    let oid = crate::utils::hex(&Sha256::digest(content));
    let object = f
        .repo
        .join(".git/lfs/objects")
        .join(&oid[..2])
        .join(&oid[2..4])
        .join(&oid);
    std::fs::create_dir_all(object.parent().unwrap()).unwrap();
    std::fs::write(&object, content).unwrap();
    let pointer = format!(
        "version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {}\n",
        content.len()
    );
    std::fs::write(f.repo.join("pkg/large.bin"), &pointer).unwrap();
    f.advance("LFS");
    let current = f.acquire().unwrap();
    assert_eq!(
        current.inputs.lfs_objects,
        vec![LfsObject {
            path: "large.bin".into(),
            oid: oid.clone(),
            size: content.len() as u64
        }]
    );
    std::fs::remove_file(&object).unwrap();
    // A validated retained snapshot is usable without re-downloading its LFS
    // payload. Moving to a new commit still requires a complete fresh export.
    assert_eq!(f.acquire().unwrap().id, current.id);
    let materialized = f
        .store
        .materialize(
            &f.app,
            None,
            &f.recipe,
            &current.id,
            &VerificationRequest {
                content: Some(false),
            },
        )
        .unwrap();
    assert_eq!(
        std::fs::read(Path::new(&materialized.path).join("large.bin")).unwrap(),
        content
    );
    f.advance("unavailable");
    let before = serde_json::to_value(f.source()).unwrap();
    assert!(f.acquire().is_err());
    assert_eq!(serde_json::to_value(f.source()).unwrap(), before);
    // Cached LFS endpoint overrides must fail even when no fetch is needed.
    let repo = f.root.join("sources").join(&f.identity.id).join("repo.git");
    git(
        &repo,
        &["config", "lfs.url", "https://denied.invalid/objects"],
    );
    let read = acquisition::read_archive(
        &f.store,
        &f.app,
        None,
        &f.recipe,
        &current.id,
        &VerificationRequest {
            content: Some(false),
        },
    );
    assert!(matches!(read, Err(e) if e.kind == ErrorKind::Authority));
    assert_eq!(serde_json::to_value(f.source()).unwrap(), before);
}

#[test]
fn installation_publishes_independent_directories_and_preserves_failed_replacements() {
    let f = Fixture::new();
    let mut recipe = f.recipe.clone();
    recipe.manifest = Some(ManifestInput {
        document: serde_json::json!({"name":"tool", "version":"1", "description":"fixture", "extra":{"keep":true}}),
        provenance: ManifestProvenance::Repository {
            path: "sauce.json".into(),
        },
    });
    let verification = VerificationRequest {
        content: Some(true),
    };
    let first = f
        .store
        .install(&f.app, None, &recipe, &verification)
        .unwrap();
    let first_path = f
        .store
        .unit_path(&f.app, None, "tool", &verification)
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(first_path.join("version.txt")).unwrap(),
        "A"
    );
    assert_eq!(first.manifest["extra"]["keep"], true);
    assert!(!first_path.join(".git").exists());
    let second_app = f._temp.path().join("app-b");
    std::fs::create_dir(&second_app).unwrap();
    let owner: Marker =
        serde_json::from_slice(&std::fs::read(f.root.join("owner.saucepanhash")).unwrap()).unwrap();
    f.store
        .register_application(
            &f.root,
            Some(&owner),
            ApplicationRegistration {
                root: second_app.to_str().unwrap().into(),
                mode: ApplicationMode::Ordinary,
                grants: vec![Grant {
                    source_id: Some(f.identity.id.clone()),
                    selection: "pkg".into(),
                    actions: [Action::Setup, Action::Inspect].into_iter().collect(),
                    destinations: vec![],
                }],
                require_verification: false,
            },
        )
        .unwrap();
    let second = f
        .store
        .install(&second_app, None, &recipe, &verification)
        .unwrap();
    let second_path = f
        .store
        .unit_path(&second_app, None, "tool", &verification)
        .unwrap();
    assert_ne!(first.id, second.id);
    assert_ne!(first_path, second_path);
    std::fs::write(first_path.join("version.txt"), "X").unwrap();
    assert_eq!(
        f.store
            .unit_path(&f.app, None, "tool", &verification)
            .unwrap_err()
            .kind,
        ErrorKind::Integrity
    );
    assert_eq!(
        f.store
            .unit_path(
                &f.app,
                None,
                "tool",
                &VerificationRequest {
                    content: Some(false)
                }
            )
            .unwrap(),
        first_path
    );
    assert_eq!(
        std::fs::read_to_string(second_path.join("version.txt")).unwrap(),
        "A"
    );
    f.advance("B");
    assert_eq!(
        f.store
            .install(&f.app, None, &recipe, &verification)
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    assert_eq!(
        f.store.read_index().unwrap().units[&first.id].artifact.id,
        first.artifact.id
    );
    assert_eq!(
        std::fs::read_to_string(first_path.join("version.txt")).unwrap(),
        "X"
    );
    std::fs::write(first_path.join("version.txt"), "A").unwrap();
    let replaced = f
        .store
        .install(&f.app, None, &recipe, &verification)
        .unwrap();
    assert_eq!(replaced.id, first.id);
    assert_ne!(replaced.artifact.id, first.artifact.id);
    assert!(!first_path.exists());
    assert_eq!(
        std::fs::read_to_string(
            f.store
                .unit_path(&f.app, None, "tool", &verification)
                .unwrap()
                .join("version.txt")
        )
        .unwrap(),
        "B"
    );
    assert_eq!(
        std::fs::read_to_string(second_path.join("version.txt")).unwrap(),
        "A"
    );
}

#[test]
fn installed_path_survives_history_eviction_but_still_checks_origin_and_scope() {
    let f = Fixture::new();
    std::fs::write(
        f.repo.join("pkg/sauce.json"),
        r#"{"name":"declared-name","version":"1","description":"repository manifest","custom":42}"#,
    )
    .unwrap();
    f.advance("B");
    let verification = VerificationRequest {
        content: Some(true),
    };
    let unit = f
        .store
        .install(&f.app, None, &f.recipe, &verification)
        .unwrap();
    for version in ["C", "D", "E", "F", "G", "H", "I"] {
        f.advance(version);
        f.acquire().unwrap();
    }
    assert!(!f.source().history.contains_key(&unit.artifact.id));
    let path = f
        .store
        .unit_path(&f.app, None, "declared-name", &verification)
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(path.join("version.txt")).unwrap(),
        "B"
    );
    assert_eq!(unit.manifest["custom"], 42);
    let absent = f
        .store
        .unit_path(&f.app, None, "missing", &verification)
        .unwrap_err();
    assert_eq!(absent.kind, ErrorKind::NotFound);
    git(
        &f.root.join(format!("sources/{}/repo.git", f.identity.id)),
        &[
            "remote",
            "set-url",
            "origin",
            "https://example.invalid/repointed.git",
        ],
    );
    assert_eq!(
        f.store
            .unit_path(
                &f.app,
                None,
                "declared-name",
                &VerificationRequest {
                    content: Some(false)
                }
            )
            .unwrap_err()
            .kind,
        ErrorKind::Integrity
    );
    assert!(path.exists());
}

#[test]
fn setup_only_scope_can_install_without_an_extra_inspect_grant() {
    let f = Fixture::new();
    let owner: Marker =
        serde_json::from_slice(&std::fs::read(f.root.join("owner.saucepanhash")).unwrap()).unwrap();
    let registration = f.store.inspect_context(&f.app, None).unwrap();
    f.store
        .update_application(
            &f.root,
            Some(&owner),
            &registration.id,
            Some(ApplicationRegistration {
                root: f.app.to_str().unwrap().into(),
                mode: ApplicationMode::Ordinary,
                grants: vec![Grant {
                    source_id: Some(f.identity.id.clone()),
                    selection: "pkg".into(),
                    actions: [Action::Setup].into_iter().collect(),
                    destinations: vec![],
                }],
                require_verification: false,
            }),
        )
        .unwrap();
    std::fs::write(
        f.repo.join("pkg/sauce.json"),
        r#"{"name":"tool","version":"1","description":"fixture"}"#,
    )
    .unwrap();
    f.advance("B");
    let verification = VerificationRequest { content: None };
    let unit = f
        .store
        .install(&f.app, None, &f.recipe, &verification)
        .unwrap();
    assert_eq!(unit.name, "tool");
    assert_eq!(
        f.store
            .unit_path(&f.app, None, "tool", &verification)
            .unwrap_err()
            .kind,
        ErrorKind::NotFound
    );
}

struct Fixture {
    _temp: tempfile::TempDir,
    store: Session,
    root: std::path::PathBuf,
    repo: std::path::PathBuf,
    app: std::path::PathBuf,
    recipe: Recipe,
    identity: SourceIdentity,
}

#[test]
fn identical_pinned_and_tracked_content_can_install_from_either_stream() {
    let f = Fixture::new();
    let tracked = f.acquire().unwrap();
    let mut pinned_recipe = f.recipe.clone();
    pinned_recipe.revision = Revision::Commit(tracked.inputs.commit.clone());
    let pinned = acquisition::acquire(
        &f.store,
        &f.app,
        None,
        &pinned_recipe,
        &VerificationRequest { content: None },
    )
    .unwrap();
    assert_eq!(tracked.id, pinned.id);
    assert_ne!(tracked.inputs.stream_id, pinned.inputs.stream_id);
    let mut recipe = if f.source().current.keys().next().unwrap() == &tracked.inputs.stream_id {
        pinned_recipe
    } else {
        f.recipe.clone()
    };
    recipe.manifest = Some(ManifestInput {
        document: serde_json::json!({"name":"tool","version":"1","description":"fixture"}),
        provenance: ManifestProvenance::LocalCatalog {
            path: "fixture".into(),
            document_digest: "fixture".into(),
        },
    });
    let installed = f
        .store
        .install(
            &f.app,
            None,
            &recipe,
            &VerificationRequest {
                content: Some(true),
            },
        )
        .unwrap();
    assert_eq!(installed.artifact.id, tracked.id);
}

#[test]
fn durable_pins_survive_eviction_and_gc_until_the_last_binding_is_removed() {
    let f = Fixture::new();
    let initial = f.acquire().unwrap();
    let verification = VerificationRequest {
        content: Some(true),
    };
    let materialized = f
        .store
        .materialize(&f.app, None, &f.recipe, &initial.id, &verification)
        .unwrap();
    let mut recipe = f.recipe.clone();
    recipe.manifest = Some(ManifestInput {
        document: serde_json::json!({"name":"tool","version":"1","description":"fixture"}),
        provenance: ManifestProvenance::LocalCatalog {
            path: "fixture".into(),
            document_digest: "fixture".into(),
        },
    });
    f.store
        .install(&f.app, None, &recipe, &verification)
        .unwrap();
    let owner: Marker =
        serde_json::from_slice(&std::fs::read(f.root.join("owner.saucepanhash")).unwrap()).unwrap();
    let app = f.store.inspect_context(&f.app, None).unwrap();
    let mut grants = app.grants;
    grants[0].actions.insert(Action::Remove);
    f.store
        .update_application(
            &f.root,
            Some(&owner),
            &app.id,
            Some(ApplicationRegistration {
                root: app.root,
                mode: ApplicationMode::Ordinary,
                grants,
                require_verification: false,
            }),
        )
        .unwrap();
    let mut acquisitions = vec![];
    for version in ["B", "C", "D", "E", "F", "G", "H"] {
        git(
            &f.repo,
            &[
                "symbolic-ref",
                "HEAD",
                &format!("refs/heads/independent-{version}"),
            ],
        );
        f.advance(version);
        acquisitions.push(f.acquire().unwrap());
    }
    assert!(!f.source().history.contains_key(&initial.id));
    let repository = f.root.join(format!("sources/{}/repo.git", f.identity.id));
    let reference = format!("refs/saucepan/artifacts/{}", initial.id);
    let refs = git(
        &repository,
        &[
            "for-each-ref",
            "--format=%(refname)",
            "refs/saucepan/artifacts/",
        ],
    );
    assert!(refs.lines().any(|name| name == reference));
    assert!(
        !refs
            .lines()
            .any(|name| name == format!("refs/saucepan/artifacts/{}", acquisitions[0].id))
    );
    // Remove only fixture clone refs so reachability proves the durable pins,
    // rather than the initial clone's unrelated branch heads, protect objects.
    for reference in git(
        &repository,
        &[
            "for-each-ref",
            "--format=%(refname)",
            "refs/heads/",
            "refs/tags/",
        ],
    )
    .lines()
    {
        git(&repository, &["update-ref", "-d", reference]);
    }
    git(&repository, &["reflog", "expire", "--expire=now", "--all"]);
    git(&repository, &["gc", "--prune=now"]);
    git(
        &repository,
        &[
            "cat-file",
            "-e",
            &format!("{}^{{commit}}", initial.inputs.commit),
        ],
    );
    assert!(
        !Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args([
                "cat-file",
                "-e",
                &format!("{}^{{commit}}", acquisitions[0].inputs.commit)
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
    f.store.uninstall(&f.app, None, "tool").unwrap();
    assert!(
        git(
            &repository,
            &["for-each-ref", "--format=%(refname)", &reference]
        )
        .lines()
        .any(|name| name == reference)
    );
    f.store
        .release_materialization(&f.app, None, &materialized.id)
        .unwrap();
    assert!(
        git(
            &repository,
            &["for-each-ref", "--format=%(refname)", &reference]
        )
        .is_empty()
    );
    git(&repository, &["reflog", "expire", "--expire=now", "--all"]);
    git(&repository, &["gc", "--prune=now"]);
    assert!(
        !Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args([
                "cat-file",
                "-e",
                &format!("{}^{{commit}}", initial.inputs.commit)
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
    git(
        &repository,
        &[
            "cat-file",
            "-e",
            &format!("{}^{{commit}}", acquisitions.last().unwrap().inputs.commit),
        ],
    );
}

#[test]
fn independent_mirrors_preserve_their_own_revision_and_require_destination_scope() {
    let f = Fixture::new();
    std::fs::write(
        f.repo.join("pkg/sauce.json"),
        r#"{"name":"tool","version":"1","description":"fixture"}"#,
    )
    .unwrap();
    f.advance("A1");
    let verification = VerificationRequest {
        content: Some(true),
    };
    let initial = f
        .store
        .install(&f.app, None, &f.recipe, &verification)
        .unwrap();
    let target = BindingTarget::Unit {
        name: "tool".into(),
    };
    let mirrors = f._temp.path().join("mirrors");
    std::fs::create_dir(&mirrors).unwrap();
    let destination = mirrors.join("tool");
    assert_eq!(
        f.store
            .mirror(&f.app, None, &target, &destination, &verification)
            .unwrap_err()
            .kind,
        ErrorKind::Authority
    );
    assert!(!destination.exists());
    let owner: Marker =
        serde_json::from_slice(&std::fs::read(f.root.join("owner.saucepanhash")).unwrap()).unwrap();
    let app = f.store.inspect_context(&f.app, None).unwrap();
    let mut grants = app.grants;
    grants[0]
        .actions
        .extend([Action::Mirror, Action::Remove, Action::Update]);
    grants[0].destinations = vec![
        std::fs::canonicalize(&mirrors)
            .unwrap()
            .to_str()
            .unwrap()
            .into(),
    ];
    f.store
        .update_application(
            &f.root,
            Some(&owner),
            &app.id,
            Some(ApplicationRegistration {
                root: app.root,
                mode: ApplicationMode::Ordinary,
                grants,
                require_verification: false,
            }),
        )
        .unwrap();
    let first = f
        .store
        .mirror(&f.app, None, &target, &destination, &verification)
        .unwrap();
    assert_eq!(first.artifact.id, initial.artifact.id);
    assert_eq!(
        std::fs::read_to_string(destination.join("version.txt")).unwrap(),
        "A1"
    );
    assert_eq!(std::fs::read_dir(&mirrors).unwrap().count(), 1);
    f.advance("B");
    let updated = f.store.update(&f.app, None, "tool", &verification).unwrap();
    assert_ne!(updated.artifact.id, first.artifact.id);
    assert_eq!(updated.mirrors[0].artifact.id, first.artifact.id);
    f.store
        .mirror_path(&f.app, None, &target, &destination, &verification)
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(destination.join("version.txt")).unwrap(),
        "A1"
    );
    std::fs::write(destination.join("version.txt"), "user edits").unwrap();
    assert_eq!(
        f.store
            .mirror(&f.app, None, &target, &destination, &verification)
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    assert_eq!(
        f.store.uninstall(&f.app, None, "tool").unwrap_err().kind,
        ErrorKind::Conflict
    );
    assert_eq!(
        std::fs::read_to_string(
            f.store
                .unit_path(&f.app, None, "tool", &verification)
                .unwrap()
                .join("version.txt")
        )
        .unwrap(),
        "B"
    );
    std::fs::write(destination.join("version.txt"), "A1").unwrap();
    let replaced = f
        .store
        .mirror(&f.app, None, &target, &destination, &verification)
        .unwrap();
    assert_eq!(replaced.artifact.id, updated.artifact.id);
    assert_eq!(
        std::fs::read_to_string(destination.join("version.txt")).unwrap(),
        "B"
    );
    assert_eq!(std::fs::read_dir(&mirrors).unwrap().count(), 1);
    let occupied = mirrors.join("unowned");
    std::fs::create_dir(&occupied).unwrap();
    std::fs::write(occupied.join("keep"), "user data").unwrap();
    assert_eq!(
        f.store
            .mirror(&f.app, None, &target, &occupied, &verification)
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    let forbidden = f._temp.path().join("forbidden");
    assert_eq!(
        f.store
            .mirror(&f.app, None, &target, &forbidden, &verification)
            .unwrap_err()
            .kind,
        ErrorKind::Authority
    );
    assert!(!forbidden.exists());
    f.store
        .remove_mirror(&f.app, None, &target, &destination)
        .unwrap();
    assert!(!destination.exists());
    assert!(
        f.store
            .unit_path(&f.app, None, "tool", &verification)
            .is_ok()
    );
    f.store
        .mirror(&f.app, None, &target, &destination, &verification)
        .unwrap();
    f.store.uninstall(&f.app, None, "tool").unwrap();
    assert!(!destination.exists());
    assert_eq!(
        std::fs::read_to_string(occupied.join("keep")).unwrap(),
        "user data"
    );
}

#[test]
fn historical_materialization_is_manifest_free_does_not_fetch_and_is_scoped() {
    let f = Fixture::new();
    let first = f.acquire().unwrap();
    f.advance("B");
    let current = f.acquire().unwrap();
    let before = f.source();
    // A refresh would fail, but materializing an exact historical descriptor
    // needs only the enrolled origin and pinned objects/ZIP.
    git(&f.repo, &["symbolic-ref", "HEAD", "refs/heads/unavailable"]);
    let verification = VerificationRequest {
        content: Some(true),
    };
    let materialized = f
        .store
        .materialize(&f.app, None, &f.recipe, &first.id, &verification)
        .unwrap();
    assert_eq!(
        materialized.artifact.disposition,
        ArtifactDisposition::Historical
    );
    assert_eq!(
        std::fs::read_to_string(Path::new(&materialized.path).join("version.txt")).unwrap(),
        "A"
    );
    assert_eq!(
        f.source().current[&current.inputs.stream_id].artifact.id,
        current.id
    );
    assert!(f.source().history[&first.id].last_used > before.history[&first.id].last_used);
    assert!(f.store.read_index().unwrap().units.is_empty());
    assert_eq!(f.store.read_index().unwrap().materializations.len(), 1);
    assert_eq!(
        f.store
            .materialization_path(&f.app, None, &materialized.id, &verification)
            .unwrap(),
        Path::new(&materialized.path)
    );
    let owner: Marker =
        serde_json::from_slice(&std::fs::read(f.root.join("owner.saucepanhash")).unwrap()).unwrap();
    let other = f._temp.path().join("other-app");
    std::fs::create_dir(&other).unwrap();
    f.store
        .register_application(
            &f.root,
            Some(&owner),
            ApplicationRegistration {
                root: other.to_str().unwrap().into(),
                mode: ApplicationMode::Ordinary,
                grants: f.store.inspect_context(&f.app, None).unwrap().grants,
                require_verification: false,
            },
        )
        .unwrap();
    assert_eq!(
        f.store
            .materialization_path(&other, None, &materialized.id, &verification)
            .unwrap_err()
            .kind,
        ErrorKind::NotFound
    );
    assert_eq!(
        f.store
            .release_materialization(&other, None, &materialized.id)
            .unwrap_err()
            .kind,
        ErrorKind::NotFound
    );
    let app = f.store.inspect_context(&f.app, None).unwrap();
    let mut grants = app.grants;
    grants[0].actions.insert(Action::Remove);
    f.store
        .update_application(
            &f.root,
            Some(&owner),
            &app.id,
            Some(ApplicationRegistration {
                root: app.root,
                mode: ApplicationMode::Ordinary,
                grants,
                require_verification: false,
            }),
        )
        .unwrap();
    f.store
        .release_materialization(&f.app, None, &materialized.id)
        .unwrap();
    assert!(!Path::new(&materialized.path).exists());
    assert!(f.store.read_index().unwrap().materializations.is_empty());
    assert_eq!(
        f.source().current[&current.inputs.stream_id].artifact.id,
        current.id
    );
}

#[test]
fn uncached_materialization_uses_exact_current_and_keeps_no_zip() {
    let f = Fixture::new();
    let first = f.acquire().unwrap();
    let owner: Marker =
        serde_json::from_slice(&std::fs::read(f.root.join("owner.saucepanhash")).unwrap()).unwrap();
    f.store
        .set_source_policy(
            &f.root,
            Some(&owner),
            &f.identity.id,
            CachePolicy {
                enabled: false,
                ..CachePolicy::default()
            },
        )
        .unwrap();
    f.advance("B");
    let uncached = f.acquire().unwrap();
    f.advance("C");
    let result = f
        .store
        .materialize(
            &f.app,
            None,
            &f.recipe,
            &uncached.id,
            &VerificationRequest {
                content: Some(true),
            },
        )
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(Path::new(&result.path).join("version.txt")).unwrap(),
        "B"
    );
    assert_eq!(result.artifact.disposition, ArtifactDisposition::Temporary);
    assert_eq!(
        f.source().current[&first.inputs.stream_id].artifact.id,
        uncached.id
    );
    assert!(
        !f.root
            .join(format!(
                "sources/{}/archives/{}.zip",
                f.identity.id, uncached.id
            ))
            .exists()
    );
}

#[test]
fn update_and_remove_use_their_own_scopes_and_preserve_failed_updates() {
    let f = Fixture::new();
    let manifest_path = f.repo.join("pkg/sauce.json");
    std::fs::write(
        &manifest_path,
        r#"{"name":"tool","version":"1","description":"fixture"}"#,
    )
    .unwrap();
    f.advance("B");
    let verification = VerificationRequest {
        content: Some(true),
    };
    let initial = f
        .store
        .install(&f.app, None, &f.recipe, &verification)
        .unwrap();
    let old_path = f
        .store
        .unit_path(&f.app, None, "tool", &verification)
        .unwrap();
    assert_eq!(
        f.store
            .update(&f.app, None, "tool", &verification)
            .unwrap_err()
            .kind,
        ErrorKind::NotFound
    );
    assert_eq!(
        f.store.uninstall(&f.app, None, "tool").unwrap_err().kind,
        ErrorKind::NotFound
    );
    let owner: Marker =
        serde_json::from_slice(&std::fs::read(f.root.join("owner.saucepanhash")).unwrap()).unwrap();
    let app = f.store.inspect_context(&f.app, None).unwrap();
    f.store
        .update_application(
            &f.root,
            Some(&owner),
            &app.id,
            Some(ApplicationRegistration {
                root: app.root,
                mode: ApplicationMode::Ordinary,
                require_verification: false,
                grants: vec![Grant {
                    source_id: Some(f.identity.id.clone()),
                    selection: "pkg".into(),
                    actions: [Action::Update, Action::Remove, Action::Inspect]
                        .into_iter()
                        .collect(),
                    destinations: vec![],
                }],
            }),
        )
        .unwrap();
    std::fs::write(
        &manifest_path,
        r#"{"name":"tool","version":"2","description":"fixture","extra":123}"#,
    )
    .unwrap();
    f.advance("C");
    let updated = f.store.update(&f.app, None, "tool", &verification).unwrap();
    assert_eq!(updated.id, initial.id);
    assert_eq!(updated.manifest["version"], "2");
    assert_eq!(updated.manifest["extra"], 123);
    assert!(!old_path.exists());
    let path = f
        .store
        .unit_path(&f.app, None, "tool", &verification)
        .unwrap();
    git(&f.repo, &["symbolic-ref", "HEAD", "refs/heads/unavailable"]);
    assert_eq!(
        f.store
            .update(&f.app, None, "tool", &verification)
            .unwrap_err()
            .kind,
        ErrorKind::Source
    );
    assert_eq!(
        f.store.read_index().unwrap().units[&initial.id].manifest["version"],
        "2"
    );
    assert_eq!(
        std::fs::read_to_string(path.join("version.txt")).unwrap(),
        "C"
    );
    git(&f.repo, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    std::fs::write(
        &manifest_path,
        r#"{"name":"renamed","version":"3","description":"fixture"}"#,
    )
    .unwrap();
    f.advance("D");
    assert_eq!(
        f.store
            .update(&f.app, None, "tool", &verification)
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    assert_eq!(f.store.read_index().unwrap().units.len(), 1);
    assert_eq!(
        f.store.read_index().unwrap().units[&initial.id].manifest["version"],
        "2"
    );
    assert_eq!(
        std::fs::read_to_string(path.join("version.txt")).unwrap(),
        "C"
    );
    std::fs::write(path.join("extra"), "user data").unwrap();
    assert_eq!(
        f.store.uninstall(&f.app, None, "tool").unwrap_err().kind,
        ErrorKind::Conflict
    );
    assert!(path.join("version.txt").exists());
    std::fs::remove_file(path.join("extra")).unwrap();
    // Missing directories are recoverable; no cache ZIP read is needed.
    for name in ["version.txt", "sauce.json", ".gitignore"] {
        std::fs::remove_file(path.join(name)).unwrap();
    }
    std::fs::remove_dir(&path).unwrap();
    f.store.uninstall(&f.app, None, "tool").unwrap();
    assert!(f.store.read_index().unwrap().units.is_empty());
    assert!(
        f.root
            .join(format!("sources/{}/repo.git", f.identity.id))
            .is_dir()
    );
    assert!(!f.source().current.is_empty());
    assert_eq!(
        f.store.uninstall(&f.app, None, "tool").unwrap_err().kind,
        ErrorKind::NotFound
    );
}

#[test]
fn same_name_different_origins_conflict_only_within_the_same_application() {
    let f = Fixture::new();
    let repo_b = f._temp.path().join("upstream-b");
    std::fs::create_dir(&repo_b).unwrap();
    git(&repo_b, &["init", "--initial-branch=main", "--template="]);
    std::fs::write(repo_b.join("file"), "other origin").unwrap();
    git(&repo_b, &["add", "."]);
    git(&repo_b, &["commit", "-m", "fixture"]);
    let mut recipe_a = f.recipe.clone();
    recipe_a.manifest = Some(ManifestInput {
        document: serde_json::json!({"name":"tool","version":"1","description":"fixture"}),
        provenance: ManifestProvenance::LocalCatalog {
            path: "fixture.json".into(),
            document_digest: "fixture".into(),
        },
    });
    let mut recipe_b = Recipe::git(repo_b.to_str().unwrap());
    recipe_b.manifest = recipe_a.manifest.clone();
    let identity_b = sources::identify(&recipe_b.source, &f.app).unwrap();
    let owner: Marker =
        serde_json::from_slice(&std::fs::read(f.root.join("owner.saucepanhash")).unwrap()).unwrap();
    let app_a = f.store.inspect_context(&f.app, None).unwrap();
    let grant_b = Grant {
        source_id: Some(identity_b.id),
        selection: ".".into(),
        actions: [Action::Setup, Action::Inspect].into_iter().collect(),
        destinations: vec![],
    };
    let mut grants = app_a.grants;
    grants.push(grant_b.clone());
    f.store
        .update_application(
            &f.root,
            Some(&owner),
            &app_a.id,
            Some(ApplicationRegistration {
                root: app_a.root,
                mode: ApplicationMode::Ordinary,
                grants,
                require_verification: false,
            }),
        )
        .unwrap();
    let verification = VerificationRequest {
        content: Some(true),
    };
    let first = f
        .store
        .install(&f.app, None, &recipe_a, &verification)
        .unwrap();
    assert_eq!(
        f.store
            .install(&f.app, None, &recipe_b, &verification)
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    );
    assert_eq!(
        f.store.read_index().unwrap().units[&first.id].artifact.id,
        first.artifact.id
    );
    let app_b = f._temp.path().join("app-b");
    std::fs::create_dir(&app_b).unwrap();
    f.store
        .register_application(
            &f.root,
            Some(&owner),
            ApplicationRegistration {
                root: app_b.to_str().unwrap().into(),
                mode: ApplicationMode::Ordinary,
                grants: vec![grant_b],
                require_verification: false,
            },
        )
        .unwrap();
    let second = f
        .store
        .install(&app_b, None, &recipe_b, &verification)
        .unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(f.store.read_index().unwrap().units.len(), 2);
    assert_eq!(
        std::fs::read_to_string(
            f.store
                .unit_path(&app_b, None, "tool", &verification)
                .unwrap()
                .join("file")
        )
        .unwrap(),
        "other origin"
    );
}
fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.autocrlf=false",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("isolated-store");
        let repo = temp.path().join("upstream");
        let app = temp.path().join("app");
        std::fs::create_dir(&repo).unwrap();
        std::fs::create_dir(&app).unwrap();
        git(&repo, &["init", "--initial-branch=main", "--template="]);
        std::fs::create_dir(repo.join("pkg")).unwrap();
        std::fs::write(
            repo.join(".gitattributes"),
            "pkg/private.txt export-ignore\n",
        )
        .unwrap();
        std::fs::write(repo.join("pkg/private.txt"), "do not export").unwrap();
        std::fs::write(repo.join("pkg/.gitignore"), "build/\n").unwrap();
        std::fs::write(repo.join("outside.txt"), "not selected").unwrap();
        std::fs::write(repo.join("pkg/version.txt"), "A").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-m", "A"]);
        let store = Session::initialize_test(&root, [41; 32]).unwrap();
        let marker: Marker =
            serde_json::from_slice(&std::fs::read(root.join("owner.saucepanhash")).unwrap())
                .unwrap();
        let mut recipe = Recipe::git(repo.to_str().unwrap());
        recipe.export.subdirectory = "pkg".into();
        let identity = sources::identify(&recipe.source, &app).unwrap();
        store
            .register_application(
                &root,
                Some(&marker),
                ApplicationRegistration {
                    root: app.to_str().unwrap().into(),
                    mode: ApplicationMode::Ordinary,
                    grants: vec![Grant {
                        source_id: Some(identity.id.clone()),
                        selection: "pkg".into(),
                        actions: [Action::Setup, Action::Inspect].into_iter().collect(),
                        destinations: vec![],
                    }],
                    require_verification: false,
                },
            )
            .unwrap();
        Self {
            _temp: temp,
            store,
            root,
            repo,
            app,
            recipe,
            identity,
        }
    }
    fn acquire(&self) -> Result<ArtifactHandle> {
        acquisition::acquire(
            &self.store,
            &self.app,
            None,
            &self.recipe,
            &VerificationRequest { content: None },
        )
    }
    fn advance(&self, version: &str) {
        std::fs::write(self.repo.join("pkg/version.txt"), version).unwrap();
        git(&self.repo, &["add", "."]);
        git(&self.repo, &["commit", "-m", version]);
    }
    fn source(&self) -> SourceIndex {
        self.store
            .generations()
            .read()
            .unwrap()
            .sources
            .remove(&self.identity.id)
            .unwrap()
    }
    fn archive(&self, id: &str) -> std::path::PathBuf {
        self.root
            .join("sources")
            .join(&self.identity.id)
            .join("archives")
            .join(format!("{id}.zip"))
    }
}

#[test]
fn real_git_subtree_rolls_five_history_behind_current_in_custom_store() {
    let f = Fixture::new();
    let mut acquired = vec![f.acquire().unwrap()];
    let same = f.acquire().unwrap();
    assert_eq!(same.id, acquired[0].id);
    assert!(f.source().history.is_empty());
    let mut zip = zip::ZipArchive::new(std::fs::File::open(f.archive(&same.id)).unwrap()).unwrap();
    let mut entries = BTreeMap::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        entries.insert(entry.name().to_owned(), bytes);
    }
    assert_eq!(
        entries.keys().map(String::as_str).collect::<Vec<_>>(),
        [".gitignore", "version.txt"]
    );
    assert_eq!(entries["version.txt"], b"A");
    drop(zip);
    for version in ["B", "C", "D", "E", "F", "G", "H"] {
        f.advance(version);
        acquired.push(f.acquire().unwrap());
    }
    let source = f.source();
    assert_eq!(source.current.len(), 1);
    assert_eq!(
        source.current.values().next().unwrap().artifact.id,
        acquired[7].id
    );
    assert_eq!(source.history.len(), 5);
    for artifact in &acquired[..2] {
        assert!(!source.history.contains_key(&artifact.id));
        assert!(!f.archive(&artifact.id).exists());
    }
    for artifact in &acquired[2..7] {
        assert!(source.history.contains_key(&artifact.id));
        assert!(f.archive(&artifact.id).is_file());
    }
    assert_eq!(
        std::fs::read_dir(f.archive(&same.id).parent().unwrap())
            .unwrap()
            .count(),
        6
    );
    Session::open_test(&f.root, [41; 32])
        .unwrap()
        .check_store()
        .unwrap();
    assert!(Session::open_test(&f.root, [42; 32]).is_err());
}

#[test]
fn tampered_outgoing_zip_and_repointed_origin_preserve_current() {
    let f = Fixture::new();
    let first = f.acquire().unwrap();
    let before = f.source();
    let original = std::fs::read(f.archive(&first.id)).unwrap();
    let mut edited = original.clone();
    edited[0] ^= 1;
    std::fs::write(f.archive(&first.id), &edited).unwrap();
    f.advance("B");
    assert!(matches!(f.acquire(), Err(e) if e.kind == ErrorKind::Integrity));
    assert_eq!(
        serde_json::to_value(f.source()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(std::fs::read(f.archive(&first.id)).unwrap(), edited);
    std::fs::write(f.archive(&first.id), original).unwrap();
    let cache = f.root.join("sources").join(&f.identity.id).join("repo.git");
    git(
        &cache,
        &[
            "remote",
            "set-url",
            "origin",
            "https://example.invalid/repointed",
        ],
    );
    assert!(matches!(f.acquire(), Err(e) if e.kind == ErrorKind::Integrity));
    assert_eq!(
        serde_json::to_value(f.source()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
}

#[test]
fn test_mode_keeps_application_scope_checks_before_source_effects() {
    let f = Fixture::new();
    let mut denied = f.recipe.clone();
    denied.export.subdirectory = ".".into();
    assert!(
        matches!(acquisition::acquire(&f.store, &f.app, None, &denied, &VerificationRequest { content: None }), Err(e) if e.kind == ErrorKind::Authority)
    );
    assert!(f.store.generations().read().unwrap().sources.is_empty());
    assert!(!f.root.join("sources").join(&f.identity.id).exists());
}

#[test]
fn two_streams_share_history_without_moving_each_others_current() {
    let f = Fixture::new();
    let first = f.acquire().unwrap();
    let mut pinned = f.recipe.clone();
    pinned.revision = Revision::Commit(first.inputs.commit.clone());
    let fixed = acquisition::acquire(
        &f.store,
        &f.app,
        None,
        &pinned,
        &VerificationRequest { content: None },
    )
    .unwrap();
    assert_eq!(fixed.id, first.id);
    assert_ne!(fixed.inputs.stream_id, first.inputs.stream_id);
    for version in ["B", "C", "D", "E", "F", "G"] {
        f.advance(version);
        f.acquire().unwrap();
    }
    let source = f.source();
    assert_eq!(source.current.len(), 2);
    assert_eq!(source.history.len(), 5);
    assert_eq!(
        source.current[&fixed.inputs.stream_id].artifact.id,
        fixed.id
    );
    assert!(!source.history.contains_key(&fixed.id));
    // Eviction removes a historical slot, not bytes still selected as current.
    assert!(f.archive(&fixed.id).is_file());
    assert_eq!(
        std::fs::read_dir(f.archive(&fixed.id).parent().unwrap())
            .unwrap()
            .count(),
        7
    );
}

#[test]
fn revocation_during_preparation_rejects_source_publication() {
    let f = Fixture::new();
    f.acquire().unwrap();
    let before = f.source();
    let access = f
        .store
        .begin_source(
            &f.app,
            None,
            &f.identity,
            "pkg",
            &VerificationRequest { content: None },
        )
        .unwrap();
    let owner: Marker =
        serde_json::from_slice(&std::fs::read(f.root.join("owner.saucepanhash")).unwrap()).unwrap();
    f.store
        .update_application(
            &f.root,
            Some(&owner),
            &f.store.inspect_context(&f.app, None).unwrap().id,
            None,
        )
        .unwrap();
    assert!(
        f.store
            .commit_source(&access, access.source.clone(), vec![])
            .is_err()
    );
    assert_eq!(
        serde_json::to_value(f.source()).unwrap(),
        serde_json::to_value(before).unwrap()
    );
}

#[test]
fn historical_read_refreshes_lru_and_checks_returned_bytes_without_fetch() {
    let f = Fixture::new();
    let a = f.acquire().unwrap();
    f.advance("B");
    let b = f.acquire().unwrap();
    for version in ["C", "D", "E", "F"] {
        f.advance(version);
        f.acquire().unwrap();
    }
    let before = f.source().current;
    f.advance("G");
    let read = acquisition::read_archive(
        &f.store,
        &f.app,
        None,
        &f.recipe,
        &a.id,
        &VerificationRequest {
            content: Some(true),
        },
    )
    .unwrap();
    assert!(read.content_rechecked);
    assert_eq!(read.bytes, std::fs::read(f.archive(&a.id)).unwrap());
    assert_eq!(
        serde_json::to_value(f.source().current).unwrap(),
        serde_json::to_value(before).unwrap()
    );
    f.acquire().unwrap();
    assert!(f.source().history.contains_key(&a.id));
    assert!(!f.source().history.contains_key(&b.id));
    assert!(!f.archive(&b.id).exists());
    let mut edited = read.bytes;
    edited[0] ^= 1;
    std::fs::write(f.archive(&a.id), &edited).unwrap();
    let before = f.source().sequence;
    assert!(
        matches!(acquisition::read_archive(&f.store, &f.app, None, &f.recipe, &a.id, &VerificationRequest { content: Some(true) }), Err(e) if e.kind == ErrorKind::Integrity)
    );
    assert_eq!(f.source().sequence, before);
    let fast = acquisition::read_archive(
        &f.store,
        &f.app,
        None,
        &f.recipe,
        &a.id,
        &VerificationRequest {
            content: Some(false),
        },
    )
    .unwrap();
    assert!(!fast.content_rechecked);
    assert_eq!(fast.bytes, edited);
}

#[test]
fn required_verification_cannot_be_disabled_before_source_enrollment() {
    let f = Fixture::new();
    let owner: Marker =
        serde_json::from_slice(&std::fs::read(f.root.join("owner.saucepanhash")).unwrap()).unwrap();
    let context = f.store.inspect_context(&f.app, None).unwrap();
    f.store
        .update_application(
            &f.root,
            Some(&owner),
            &context.id,
            Some(ApplicationRegistration {
                root: context.root.clone(),
                mode: context.mode,
                grants: context.grants.clone(),
                require_verification: true,
            }),
        )
        .unwrap();
    let generation = f.store.read_index().unwrap().generation;
    assert!(
        matches!(acquisition::acquire(&f.store, &f.app, None, &f.recipe, &VerificationRequest { content: Some(false) }), Err(e) if e.kind == ErrorKind::Authority)
    );
    assert_eq!(f.store.read_index().unwrap().generation, generation);
    assert!(f.store.generations().read().unwrap().sources.is_empty());
    assert!(!f.root.join("sources").join(&f.identity.id).exists());
}

#[test]
fn committed_export_attributes_ignore_local_overrides() {
    let f = Fixture::new();
    std::fs::write(
        f.repo.join(".gitattributes"),
        "pkg/private.txt export-ignore\npkg/stamp.txt export-subst\n",
    )
    .unwrap();
    std::fs::write(f.repo.join("pkg/stamp.txt"), "$Format:%H$\n").unwrap();
    f.advance("B");
    let acquired = f.acquire().unwrap();
    let mut zip =
        zip::ZipArchive::new(std::fs::File::open(f.archive(&acquired.id)).unwrap()).unwrap();
    let mut stamp = String::new();
    zip.by_name("stamp.txt")
        .unwrap()
        .read_to_string(&mut stamp)
        .unwrap();
    assert_eq!(stamp.trim(), acquired.inputs.commit);
    assert!(zip.by_name("private.txt").is_err());
    drop(zip);
    let cache = f.root.join("sources").join(&f.identity.id).join("repo.git");
    std::fs::create_dir_all(cache.join("info")).unwrap();
    std::fs::write(
        cache.join("info/attributes"),
        "pkg/private.txt -export-ignore\npkg/stamp.txt -export-subst\n",
    )
    .unwrap();
    let mut pinned = f.recipe.clone();
    pinned.revision = Revision::Commit(acquired.inputs.commit.clone());
    let rebuilt = acquisition::acquire(
        &f.store,
        &f.app,
        None,
        &pinned,
        &VerificationRequest { content: None },
    )
    .unwrap();
    assert_eq!(rebuilt.id, acquired.id);
    assert_eq!(rebuilt.archive_digest, acquired.archive_digest);
    assert_eq!(rebuilt.tree_digest, acquired.tree_digest);
}

#[test]
fn source_retention_is_managed_and_reenable_preserves_exact_uncached_current() {
    let f = Fixture::new();
    let a = f.acquire().unwrap();
    let owner: Marker =
        serde_json::from_slice(&std::fs::read(f.root.join("owner.saucepanhash")).unwrap()).unwrap();
    let disabled = CachePolicy {
        enabled: false,
        ..CachePolicy::default()
    };
    assert!(
        matches!(f.store.set_source_policy(&f.app, None, &f.identity.id, disabled.clone()), Err(e) if e.kind == ErrorKind::Authority)
    );
    f.store
        .set_source_policy(&f.root, Some(&owner), &f.identity.id, disabled)
        .unwrap();
    f.advance("B");
    let b = f.acquire().unwrap();
    assert_eq!(b.disposition, ArtifactDisposition::Temporary);
    assert!(f.source().history.is_empty());
    assert!(!f.archive(&a.id).exists());
    assert!(!f.archive(&b.id).exists());
    f.advance("C");
    let bytes = acquisition::read_archive(
        &f.store,
        &f.app,
        None,
        &f.recipe,
        &b.id,
        &VerificationRequest {
            content: Some(true),
        },
    )
    .unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes.bytes)).unwrap();
    let mut version = String::new();
    zip.by_name("version.txt")
        .unwrap()
        .read_to_string(&mut version)
        .unwrap();
    assert_eq!(version, "B");
    assert!(!f.archive(&b.id).exists());
    f.store
        .set_source_policy(
            &f.root,
            Some(&owner),
            &f.identity.id,
            CachePolicy::default(),
        )
        .unwrap();
    let c = f.acquire().unwrap();
    assert_eq!(c.evidence.policy_revision, 3);
    assert!(f.source().history.contains_key(&b.id));
    assert!(f.archive(&b.id).is_file());
    assert!(f.archive(&c.id).is_file());
    assert_ne!(c.inputs.commit, b.inputs.commit);
}

#[test]
fn eviction_skips_busy_lru_and_five_process_readers_preserve_previous_current() {
    let f = Fixture::new();
    let a = f.acquire().unwrap();
    f.advance("B");
    let b = f.acquire().unwrap();
    for version in ["C", "D", "E", "F"] {
        f.advance(version);
        f.acquire().unwrap();
    }
    let lease = Lock::shared(
        &f.store.layout,
        &format!("archive-{}-{}", f.identity.id, a.id),
        Instant::now(),
    )
    .unwrap();
    f.advance("G");
    f.acquire().unwrap();
    assert!(f.source().history.contains_key(&a.id));
    assert!(!f.source().history.contains_key(&b.id));
    drop(lease);
    let before = f.source();
    let names = before
        .history
        .keys()
        .map(|id| format!("archive-{}-{id}", f.identity.id))
        .collect::<Vec<_>>()
        .join("\n");
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "core::store::locking::tests::process_worker",
            "--ignored",
        ])
        .env("SAUCEPAN_LOCK_TEST_HOME", &f.root)
        .env("SAUCEPAN_LOCK_TEST_MODE", "archive-readers")
        .env("SAUCEPAN_ARCHIVE_LEASE_TEST_NAMES", names)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !f.root.join("readers-ready").exists() {
        assert!(Instant::now() < deadline, "reader fixture startup deadline");
        std::thread::sleep(Duration::from_millis(10));
    }
    f.advance("H");
    assert!(matches!(f.acquire(), Err(e) if e.kind == ErrorKind::Busy));
    assert_eq!(
        serde_json::to_value(f.source()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    for id in before.history.keys() {
        assert!(f.archive(id).is_file());
    }
    std::fs::write(f.root.join("readers-release"), b"release").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    f.acquire().unwrap();
    assert_eq!(f.source().history.len(), 5);
}

#[test]
fn different_subdirectory_streams_share_one_five_entry_history_pool() {
    let f = Fixture::new();
    std::fs::create_dir(f.repo.join("pkg/sub")).unwrap();
    std::fs::write(f.repo.join("pkg/sub/item"), "selected child").unwrap();
    f.advance("B");
    let mut child = f.recipe.clone();
    child.export.subdirectory = "pkg/sub".into();
    let root = f.acquire().unwrap();
    let sub = acquisition::acquire(
        &f.store,
        &f.app,
        None,
        &child,
        &VerificationRequest { content: None },
    )
    .unwrap();
    assert_ne!(root.id, sub.id);
    let mut last_root = root.id;
    let mut last_sub = sub.id;
    for (n, version) in ["C", "D", "E", "F", "G", "H", "I"].iter().enumerate() {
        f.advance(version);
        if n % 2 == 0 {
            last_root = f.acquire().unwrap().id;
            assert_eq!(
                f.source().current[&sub.inputs.stream_id].artifact.id,
                last_sub
            );
        } else {
            last_sub = acquisition::acquire(
                &f.store,
                &f.app,
                None,
                &child,
                &VerificationRequest { content: None },
            )
            .unwrap()
            .id;
            assert_eq!(
                f.source().current[&root.inputs.stream_id].artifact.id,
                last_root
            );
        }
    }
    assert_eq!(f.source().current.len(), 2);
    assert_eq!(f.source().history.len(), 5);
    assert_eq!(
        std::fs::read_dir(f.archive(&last_sub).parent().unwrap())
            .unwrap()
            .count(),
        7
    );
}
