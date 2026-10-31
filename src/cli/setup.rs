use super::{Args, Commands};
use crate::{
    core::{setup, source::AcquireOptions},
    models::index::lifecycle::Phase,
};
use anyhow::{Result, ensure};

#[derive(clap::Args)]
pub(super) struct SetupArgs {
    built_shim: std::path::PathBuf,
    #[arg(long)]
    tag: String,
    #[arg(long)]
    operation: Option<uuid::Uuid>,
    #[arg(long)]
    migrate_unmanaged: bool,
    #[arg(long)]
    expected_sha256: Option<String>,
}

pub(super) fn run(args: &Args) -> Result<()> {
    run_inner(args).map_err(|error| {
        if error.downcast_ref::<crate::error::Error>().is_some() {
            error
        } else {
            // Setup's located refusals contain caller-supplied paths and public inputs,
            // never credentials. Preserve them through the CLI's safe error boundary.
            let code = if error.downcast_ref::<std::io::Error>().is_some() {
                crate::error::ErrorCode::IoError
            } else {
                crate::error::ErrorCode::InvalidRequest
            };
            let message = format!("{error:#}");
            error.context(crate::error::Error::new(code, message))
        }
    })
}

fn run_inner(args: &Args) -> Result<()> {
    ensure!(
        args.as_app.is_none() && args.credential_file.is_none(),
        crate::error::Error::new(
            crate::error::ErrorCode::AccessDenied,
            "setup requires trusted administration"
        )
    );
    ensure!(
        args.portable_policies.is_none(),
        "setup does not accept portable policies"
    );
    let root = super::root::resolve(args.root.clone())?;
    let mut code = 0;
    let value = match &args.command {
        Commands::SetupStatus => serde_json::to_value(setup::inspect(&root)?)?,
        Commands::Setup(input) => {
            let SetupArgs {
                built_shim,
                tag,
                operation,
                migrate_unmanaged,
                expected_sha256,
            } = input.as_ref();
            let result = setup::setup(
                &root,
                &setup::Options {
                    built_shim: built_shim.clone(),
                    tag: tag.clone(),
                    operation: *operation,
                    migrate_unmanaged: *migrate_unmanaged,
                    expected_sha256: expected_sha256.clone(),
                },
                &AcquireOptions {
                    test_now: args.test_now,
                    test_github_api: args.test_github_api.clone(),
                    ..Default::default()
                },
            )?;
            if result.status != Phase::Complete {
                code = 2;
            }
            serde_json::to_value(result)?
        }
        _ => unreachable!(),
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}
