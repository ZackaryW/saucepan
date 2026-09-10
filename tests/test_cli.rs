use saucepan::core::{Store, models::*};
use std::{
    path::PathBuf,
    process::{Command, Output},
};

struct Fixture {
    temp: tempfile::TempDir,
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::Builder::new().prefix("sc-").tempdir().unwrap();
        let root = temp.path().join("store");
        Self { temp, root }
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_saucepan"))
            .args([
                "--test-root",
                self.root.to_str().unwrap(),
                "--test-key",
                &"07".repeat(32),
            ])
            .args(args)
            .output()
            .unwrap()
    }
    fn json(&self, args: &[&str]) -> serde_json::Value {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

#[test]
fn real_binary_and_native_api_share_settings_scoped_views_and_stable_markers() {
    let f = Fixture::new();
    f.json(&["init"]);
    let proof: AppToken = serde_json::from_value(f.json(&["register", "app"])).unwrap();
    let marker = f.temp.path().join(".saucepanhash");
    let native = Store::open_test(&f.root, [7; 32]).unwrap();
    native.write_marker(&proof, &marker).unwrap();
    let marker_bytes = std::fs::read(&marker).unwrap();
    let marker_path = marker.to_str().unwrap();
    let input = f.temp.path().join("asset");
    std::fs::write(&input, "content").unwrap();
    let recipe = f.temp.path().join("recipe.json");
    std::fs::write(
        &recipe,
        serde_json::to_vec(&Recipe {
            source: Source::Local { path: input },
            folder: None,
            commit: None,
        })
        .unwrap(),
    )
    .unwrap();
    let acquired: Acquired = serde_json::from_value(f.json(&[
        "--marker",
        marker_path,
        "acquire",
        recipe.to_str().unwrap(),
    ]))
    .unwrap();
    assert_eq!(
        std::fs::read(acquired.directory.join("asset")).unwrap(),
        b"content"
    );
    let app = AppContext::authenticated(proof);
    let view = f.json(&["--marker", marker_path, "view"]);
    assert_eq!(
        view,
        serde_json::to_value(native.view(&app).unwrap()).unwrap()
    );
    let saved = f.temp.path().join("view.json");
    std::fs::write(&saved, serde_json::to_vec(&view).unwrap()).unwrap();
    f.json(&["--marker", marker_path, "verify", saved.to_str().unwrap()]);
    native
        .configure(
            &app,
            AppSettings {
                verify_content: true,
                ..AppSettings::default()
            },
            Filters::default(),
        )
        .unwrap();
    assert_eq!(
        f.run(&["--marker", marker_path, "verify", saved.to_str().unwrap()])
            .status
            .code(),
        Some(1)
    );
    assert_eq!(
        f.json(&["--marker", marker_path, "view"])["settings"]["verify_content"],
        true
    );
    assert_eq!(std::fs::read(marker).unwrap(), marker_bytes);
    f.json(&["register", "other"]);
    assert!(
        f.json(&["--app", "other", "path", &acquired.artifact.id])
            .is_null()
    );
    assert_eq!(
        f.run(&["--app", "app", "--authoritative", "view"])
            .status
            .code(),
        Some(1)
    );
}
