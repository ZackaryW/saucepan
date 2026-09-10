use std::process::Command;

#[test]
fn explicit_test_flags_require_both_values_in_the_normal_binary() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("unused");
    let output = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("--test-root"));
    assert!(!help.contains("--test-key")); // Secret-bearing option is hidden from help.
    for args in [
        vec!["--test-root", root.to_str().unwrap(), "init"],
        vec!["--test-key", "00", "init"],
        vec!["--test-store", root.to_str().unwrap(), "store", "check"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_saucepan"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!root.exists());
    }
    let output = Command::new(env!("CARGO_BIN_EXE_saucepan"))
        .args([
            "--test-root",
            root.to_str().unwrap(),
            "--test-key",
            "invalid",
            "init",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!output.stderr.is_empty());
    assert!(!root.exists());
}
