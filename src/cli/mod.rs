use crate::models::store::VerificationMode;
use crate::{core::Store, models::source::Source};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version = concat!(env!("CARGO_PKG_VERSION"),"+",env!("SAUCEPAN_BUILD_ID")))]
struct Args {
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    #[arg(long, global = true)]
    force_refresh: bool,
    #[arg(long, global = true)]
    allow_stale: bool,
    #[arg(long, global = true, hide = true)]
    test_now: Option<u64>,
    #[arg(long, global = true, hide = true)]
    test_github_api: Option<String>,
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    Bootstrap {
        request: PathBuf,
        built_shim: PathBuf,
        #[arg(long)]
        operation: Option<uuid::Uuid>,
    },
    SelfUpdate {
        app: uuid::Uuid,
        claim: uuid::Uuid,
        #[arg(long)]
        operation: Option<uuid::Uuid>,
    },
    Install {
        name: String,
        request: PathBuf,
        probe: PathBuf,
        built_shim: PathBuf,
        #[arg(long)]
        shim: String,
        #[arg(long)]
        operation: Option<uuid::Uuid>,
    },
    UpdateApp {
        app: uuid::Uuid,
        claim: uuid::Uuid,
        probe: PathBuf,
        #[arg(long)]
        operation: Option<uuid::Uuid>,
    },
    Rollback {
        app: uuid::Uuid,
        claim: uuid::Uuid,
        entry: uuid::Uuid,
        probe: PathBuf,
        #[arg(long)]
        operation: Option<uuid::Uuid>,
    },
    /// Deterministic acceptance fixture: keep this actual build executing.
    #[command(hide = true)]
    TestHold {
        ready: PathBuf,
        release: PathBuf,
    },
    Init,
    /// List durable lifecycle operations, including pending cleanup.
    Operations {
        #[arg(long)]
        app: Option<uuid::Uuid>,
    },
    /// Resume recorded lifecycle work without inferring ownership.
    Reconcile {
        #[arg(long)]
        operation: Option<uuid::Uuid>,
    },
    RegisterApp {
        name: String,
    },
    RegisterSource {
        file: PathBuf,
    },
    Inspect {
        uuid: uuid::Uuid,
    },
    Acquire {
        file: PathBuf,
        #[arg(long)]
        app: Option<uuid::Uuid>,
    },
    UpdateClaim {
        app: uuid::Uuid,
        claim: uuid::Uuid,
    },
    ReleaseClaim {
        app: uuid::Uuid,
        claim: uuid::Uuid,
    },
    Claims {
        app: uuid::Uuid,
    },
    Settings {
        app: uuid::Uuid,
        file: PathBuf,
    },
    Path {
        uuid: uuid::Uuid,
        #[arg(long, value_enum, default_value = "metadata")]
        verification: VerificationMode,
    },
    Verify {
        uuid: uuid::Uuid,
    },
    /// Run a consumer with SAUCEPAN_RESOURCE_DIR and a native lifetime lease.
    Run {
        uuid: uuid::Uuid,
        #[arg(long, value_enum, default_value = "metadata")]
        verification: VerificationMode,
        #[arg(last = true, required = true, num_args = 1..)]
        command: Vec<std::ffi::OsString>,
    },
    Reclaim,
    Reacquire {
        uuid: uuid::Uuid,
    },
    Activate {
        app: uuid::Uuid,
        entry: uuid::Uuid,
        executable: String,
        probe: PathBuf,
    },
    /// Clear an application's active binding; preserve claims and running consumers.
    Deactivate {
        app: uuid::Uuid,
    },
    Uninstall {
        app: uuid::Uuid,
        #[arg(long)]
        leave_unmanaged_shims: bool,
        #[arg(long)]
        operation: Option<uuid::Uuid>,
    },
    InstallShim {
        app: uuid::Uuid,
        name: String,
        built_shim: PathBuf,
        #[arg(long)]
        saucepan: bool,
        #[arg(long)]
        operation: Option<uuid::Uuid>,
    },
    ReplaceShim {
        app: uuid::Uuid,
        name: String,
        built_shim: PathBuf,
        #[arg(long)]
        saucepan: bool,
        #[arg(long)]
        operation: Option<uuid::Uuid>,
    },
    AdoptShim {
        app: uuid::Uuid,
        name: String,
        built_shim: PathBuf,
        #[arg(long)]
        saucepan: bool,
        #[arg(long)]
        operation: Option<uuid::Uuid>,
    },
}
pub fn run() -> Result<()> {
    let args = Args::parse();
    let root = args.root.context("--root is required")?;
    let options = crate::core::source::AcquireOptions {
        force_refresh: args.force_refresh,
        allow_stale: args.allow_stale,
        test_now: args.test_now,
        test_github_api: args.test_github_api,
    };
    let mut exit_code = 0;
    let value = match args.command {
        Commands::Init => {
            let store = Store::create(&root)?;
            serde_json::json!({"root":store.root()})
        }
        command => {
            let store = Store::open(&root)?;
            match command {
                Commands::Operations { app } => serde_json::to_value(store.operations(app)?)?,
                Commands::Reconcile { operation } => {
                    let results = store.reconcile(operation)?;
                    if results
                        .iter()
                        .any(|op| op.status != crate::models::index::lifecycle::Phase::Complete)
                    {
                        exit_code = 2;
                    }
                    serde_json::to_value(results)?
                }
                Commands::RegisterApp { name } => serde_json::to_value(store.register_app(name)?)?,
                Commands::Bootstrap {
                    request,
                    built_shim,
                    operation,
                } => operation_value(
                    store.bootstrap_with_operation(
                        &serde_json::from_slice(&std::fs::read(request)?)?,
                        &built_shim,
                        &options,
                        operation,
                    )?,
                    &mut exit_code,
                )?,
                Commands::SelfUpdate {
                    app,
                    claim,
                    operation,
                } => operation_value(
                    store.self_update_with_operation(app, claim, &options, operation)?,
                    &mut exit_code,
                )?,
                Commands::Install {
                    name,
                    request,
                    probe,
                    built_shim,
                    shim,
                    operation,
                } => operation_value(
                    store.install_application(
                        &crate::models::index::lifecycle::Installation {
                            name,
                            request: serde_json::from_slice(&std::fs::read(request)?)?,
                            probe: serde_json::from_slice(&std::fs::read(probe)?)?,
                            shim,
                            saucepan: false,
                        },
                        &built_shim,
                        &options,
                        operation,
                    )?,
                    &mut exit_code,
                )?,
                Commands::UpdateApp {
                    app,
                    claim,
                    probe,
                    operation,
                } => operation_value(
                    store.update_application(
                        app,
                        claim,
                        &serde_json::from_slice(&std::fs::read(probe)?)?,
                        None,
                        &options,
                        operation,
                    )?,
                    &mut exit_code,
                )?,
                Commands::Rollback {
                    app,
                    claim,
                    entry,
                    probe,
                    operation,
                } => operation_value(
                    store.update_application(
                        app,
                        claim,
                        &serde_json::from_slice(&std::fs::read(probe)?)?,
                        Some(entry),
                        &options,
                        operation,
                    )?,
                    &mut exit_code,
                )?,
                Commands::TestHold { ready, release } => {
                    std::fs::write(&ready, env!("SAUCEPAN_BUILD_ID"))?;
                    let start = std::time::Instant::now();
                    while !release.exists() && start.elapsed() < std::time::Duration::from_secs(120)
                    {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    serde_json::json!({"build":env!("SAUCEPAN_BUILD_ID"),"root":store.root()})
                }
                Commands::RegisterSource { file } => {
                    let source: Source = serde_json::from_slice(&std::fs::read(file)?)?;
                    serde_json::to_value(store.register_source(source)?)?
                }
                Commands::Inspect { uuid } => serde_json::to_value(store.record(uuid)?)?,
                Commands::Acquire { file, app } => {
                    let request = serde_json::from_slice(&std::fs::read(file)?)?;
                    match app {
                        Some(app) => {
                            serde_json::to_value(store.acquire_for(app, &request, &options)?)?
                        }
                        None => serde_json::to_value(store.acquire_with(&request, &options)?)?,
                    }
                }
                Commands::UpdateClaim { app, claim } => {
                    serde_json::to_value(store.update_claim(app, claim, &options)?)?
                }
                Commands::ReleaseClaim { app, claim } => {
                    store.release_claim(app, claim)?;
                    serde_json::json!({"released":claim})
                }
                Commands::Claims { app } => serde_json::to_value(store.claims(app)?)?,
                Commands::Settings { app, file } => {
                    store.set_settings(app, serde_json::from_slice(&std::fs::read(file)?)?)?;
                    serde_json::json!({"updated":app})
                }
                Commands::Path { uuid, verification } => {
                    serde_json::to_value(store.resolve(uuid, verification)?)?
                }
                Commands::Verify { uuid } => serde_json::to_value(store.verify(uuid)?)?,
                Commands::Run {
                    uuid,
                    verification,
                    command,
                } => {
                    let lease = store.lease(uuid, verification)?;
                    let mut process = std::process::Command::new(&command[0]);
                    process
                        .args(&command[1..])
                        .env("SAUCEPAN_RESOURCE_DIR", lease.directory());
                    let code = crate::core::launch::launch_leased(process, lease)?;
                    std::process::exit(code);
                }
                Commands::Reclaim => serde_json::to_value(store.reclaim()?)?,
                Commands::Reacquire { uuid } => {
                    serde_json::to_value(store.reacquire(uuid, &options)?)?
                }
                Commands::Activate {
                    app,
                    entry,
                    executable,
                    probe,
                } => serde_json::to_value(store.activate(
                    app,
                    entry,
                    &executable,
                    &serde_json::from_slice(&std::fs::read(probe)?)?,
                )?)?,
                Commands::Deactivate { app } => {
                    let previous = store.deactivate(app)?;
                    serde_json::json!({
                        "app": app,
                        "deactivated": previous.is_some(),
                        "previous_binding": previous,
                    })
                }
                Commands::Uninstall {
                    app,
                    leave_unmanaged_shims,
                    operation,
                } => operation_value(
                    store.uninstall(app, leave_unmanaged_shims, operation)?,
                    &mut exit_code,
                )?,
                Commands::InstallShim {
                    app,
                    name,
                    built_shim,
                    saucepan,
                    operation,
                } => operation_value(
                    store.maintain_shim(
                        app,
                        &name,
                        &built_shim,
                        crate::models::index::lifecycle::ShimAction::Install,
                        saucepan,
                        operation,
                    )?,
                    &mut exit_code,
                )?,
                Commands::ReplaceShim {
                    app,
                    name,
                    built_shim,
                    saucepan,
                    operation,
                } => operation_value(
                    store.maintain_shim(
                        app,
                        &name,
                        &built_shim,
                        crate::models::index::lifecycle::ShimAction::Replace,
                        saucepan,
                        operation,
                    )?,
                    &mut exit_code,
                )?,
                Commands::AdoptShim {
                    app,
                    name,
                    built_shim,
                    saucepan,
                    operation,
                } => operation_value(
                    store.maintain_shim(
                        app,
                        &name,
                        &built_shim,
                        crate::models::index::lifecycle::ShimAction::Adopt,
                        saucepan,
                        operation,
                    )?,
                    &mut exit_code,
                )?,
                Commands::Init => unreachable!(),
            }
        }
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

fn operation_value(
    op: crate::models::index::lifecycle::Operation,
    exit_code: &mut i32,
) -> Result<serde_json::Value> {
    if op.status != crate::models::index::lifecycle::Phase::Complete {
        *exit_code = 2;
    }
    Ok(serde_json::to_value(op)?)
}
