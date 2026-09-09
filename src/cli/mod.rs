//! Thin JSON/file adapter around the same core API used by native callers.
mod commands;
use crate::core::{Store, models::AppContext};
use anyhow::{Context, Result, ensure};
use clap::Parser;
use std::{ffi::OsString, path::PathBuf};

#[derive(Parser)]
#[command(
    name = "saucepan",
    version,
    about = "Shared source acquisition with encrypted app records"
)]
struct Arguments {
    #[arg(long, global = true)]
    app: Option<String>,
    #[arg(long, global = true)]
    marker: Option<PathBuf>,
    #[arg(long, global = true)]
    authoritative: bool,
    #[arg(long, global = true, requires = "test_key")]
    test_root: Option<PathBuf>,
    #[arg(long, global = true, requires = "test_root", hide = true)]
    test_key: Option<String>,
    #[command(subcommand)]
    command: commands::Command,
}

pub fn run_from<I, T>(args: I) -> Result<serde_json::Value>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let args = Arguments::try_parse_from(args)?;
    if matches!(args.command, commands::Command::SharedExecutable) {
        return Ok(serde_json::to_value(
            crate::core::platform::shared_executable()?,
        )?);
    }
    let create = matches!(args.command, commands::Command::Init);
    let store = if let Some(root) = args.test_root {
        let encoded = args
            .test_key
            .context("custom test path requires a test key")?;
        ensure!(
            encoded.len() == 64 && encoded.is_ascii(),
            "test key must be 64 hex characters"
        );
        let mut key = zeroize::Zeroizing::new([0; 32]);
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&encoded[i * 2..i * 2 + 2], 16)
                .context("test key must be hexadecimal")?;
        }
        if create {
            Store::create_test(root, *key)?
        } else {
            Store::open_test(root, *key)?
        }
    } else if create {
        Store::create_user()?
    } else {
        Store::open_user()?
    };
    let proof = args.marker.map(Store::read_marker).transpose()?;
    let app = args
        .app
        .or_else(|| proof.as_ref().map(|p| p.app.clone()))
        .unwrap_or_default();
    let context = AppContext {
        authoritative: args.authoritative || proof.is_some(),
        proof,
        ..AppContext::ordinary(app)
    };
    commands::execute(&store, &context, args.command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Store, models::*};
    use std::fs;

    #[test]
    fn cli_and_native_share_registration_settings_acquisition_and_verification() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("store");
        let invoke = |tail: &[&str]| {
            let mut args = vec![
                "saucepan".to_owned(),
                "--test-root".into(),
                root.to_str().unwrap().into(),
                "--test-key".into(),
                "07".repeat(32),
            ];
            args.extend(tail.iter().map(|s| s.to_string()));
            run_from(args)
        };
        invoke(&["init"]).unwrap();
        let proof: AppToken =
            serde_json::from_value(invoke(&["register", "app"]).unwrap()).unwrap();
        let marker = dir.path().join(".saucepanhash");
        let store = Store::open_test(&root, [7; 32]).unwrap();
        store.write_marker(&proof, &marker).unwrap();
        let input = dir.path().join("input");
        fs::create_dir(&input).unwrap();
        fs::write(input.join("file"), b"content").unwrap();
        let recipe = dir.path().join("recipe.json");
        fs::write(
            &recipe,
            serde_json::to_vec(&Recipe {
                source: Source::Local { path: input },
                folder: None,
                commit: None,
            })
            .unwrap(),
        )
        .unwrap();
        let result: Acquired = serde_json::from_value(
            invoke(&[
                "--marker",
                marker.to_str().unwrap(),
                "acquire",
                recipe.to_str().unwrap(),
            ])
            .unwrap(),
        )
        .unwrap();
        assert_eq!(fs::read(result.directory.join("file")).unwrap(), b"content");
        let context = AppContext::authenticated(proof);
        let native = store.view(&context).unwrap();
        let from_cli: AppView = serde_json::from_value(
            invoke(&["--marker", marker.to_str().unwrap(), "view"]).unwrap(),
        )
        .unwrap();
        assert_eq!(native, from_cli);
        let view = dir.path().join("view.json");
        fs::write(&view, serde_json::to_vec(&native).unwrap()).unwrap();
        invoke(&[
            "--marker",
            marker.to_str().unwrap(),
            "verify",
            view.to_str().unwrap(),
        ])
        .unwrap();
        let settings = dir.path().join("settings.json");
        fs::write(
            &settings,
            r#"{"retain_snapshots":false,"verify_content":true,"allow_local_fallback":false}"#,
        )
        .unwrap();
        invoke(&[
            "--marker",
            marker.to_str().unwrap(),
            "configure",
            "--settings",
            settings.to_str().unwrap(),
        ])
        .unwrap();
        assert!(store.view(&context).unwrap().settings.verify_content);
        assert!(invoke(&["--app", "app", "--authoritative", "view"]).is_err());
        assert!(
            invoke(&[
                "--marker",
                marker.to_str().unwrap(),
                "verify",
                view.to_str().unwrap()
            ])
            .is_err()
        );
    }

    #[test]
    fn custom_keys_require_explicit_custom_paths_and_reject_invalid_keys() {
        assert!(run_from(["saucepan", "--test-key", "00", "init"]).is_err());
        assert!(run_from(["saucepan", "--test-root", "unused", "init"]).is_err());
        assert!(
            run_from([
                "saucepan",
                "--test-root",
                "unused",
                "--test-key",
                "invalid",
                "init"
            ])
            .is_err()
        );
        assert!(run_from(["saucepan", "unrecognized-command"]).is_err());
    }
}
