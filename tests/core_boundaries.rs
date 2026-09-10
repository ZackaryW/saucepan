use std::{
    fs,
    path::{Path, PathBuf},
};

fn rust_files(root: &Path, out: &mut Vec<PathBuf>) {
    for item in fs::read_dir(root).unwrap() {
        let path = item.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn core_dependency_direction_and_passive_records() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core");
    let mut files = vec![];
    rust_files(&root, &mut files);
    for path in files {
        let text = fs::read_to_string(&path).unwrap();
        // Production imports only; tests may create fixtures.
        let production = text.split("#[cfg(test)]").next().unwrap();
        for forbidden in [
            "crate::cli",
            "crate::commands",
            "crate::config",
            "crate::index",
            "crate::bucket",
            "crate::sources",
            "println!",
            "eprintln!",
            "include!(",
        ] {
            assert!(
                !production.contains(forbidden),
                "{} imports legacy/CLI or prints: {forbidden}",
                path.display()
            );
        }
        // Recipe canonicalization explicitly resolves local paths; record definitions do not.
        if path.file_name().is_some_and(|name| name != "recipe.rs")
            && path
                .components()
                .any(|part| part.as_os_str() == "models" || part.as_os_str() == "policies")
        {
            for forbidden in [
                "std::fs",
                "std::process",
                "std::net",
                "std::env",
                "tokio::",
                "reqwest::",
                "keyring::",
                "super::store",
                "super::sources",
            ] {
                assert!(
                    !production.contains(forbidden),
                    "{} has hidden I/O: {forbidden}",
                    path.display()
                );
            }
        }
    }
    let leaf = fs::read_to_string(root.parent().unwrap().join("utils/mod.rs")).unwrap();
    for forbidden in ["crate::core", "std::fs", "std::process", "std::net"] {
        assert!(
            !leaf.contains(forbidden),
            "leaf helpers depend on domain/I/O"
        );
    }
}

#[test]
fn root_targets_do_not_compile_the_legacy_reference() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = std::process::Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success());
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    for package in metadata["packages"].as_array().unwrap() {
        for target in package["targets"].as_array().unwrap() {
            let path = Path::new(target["src_path"].as_str().unwrap());
            assert!(
                !path.components().any(|part| ["src2", "src3"]
                    .iter()
                    .any(|name| part.as_os_str() == *name)),
                "legacy target: {}",
                path.display()
            );
        }
    }
    let mut files = vec![];
    rust_files(&root.join("src"), &mut files);
    for path in files {
        let text = fs::read_to_string(&path).unwrap();
        for line in text
            .lines()
            .filter(|line| line.contains("#[path") || line.contains("include!("))
        {
            assert!(
                !line.contains("src2") && !line.contains("src3"),
                "legacy source inclusion: {}",
                path.display()
            );
        }
    }
}
