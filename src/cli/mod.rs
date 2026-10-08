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
    },
    SelfUpdate {
        app: uuid::Uuid,
        claim: uuid::Uuid,
    },
    /// Deterministic acceptance fixture: keep this actual build executing.
    #[command(hide = true)]
    TestHold {
        ready: PathBuf,
        release: PathBuf,
    },
    Init,
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
    InstallShim {
        app: uuid::Uuid,
        name: String,
        built_shim: PathBuf,
        #[arg(long)]
        saucepan: bool,
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
    let value = match args.command {
        Commands::Init => {
            let store = Store::create(&root)?;
            serde_json::json!({"root":store.root()})
        }
        command => {
            let store = Store::open(&root)?;
            match command {
                Commands::RegisterApp { name } => serde_json::to_value(store.register_app(name)?)?,
                Commands::Bootstrap {
                    request,
                    built_shim,
                } => serde_json::to_value(store.bootstrap(
                    &serde_json::from_slice(&std::fs::read(request)?)?,
                    &built_shim,
                    &options,
                )?)?,
                Commands::SelfUpdate { app, claim } => {
                    serde_json::to_value(store.self_update(app, claim, &options)?)?
                }
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
                Commands::InstallShim {
                    app,
                    name,
                    built_shim,
                    saucepan,
                } => {
                    serde_json::json!({"shim":store.install_shim(app,&name,&built_shim,saucepan)?})
                }
                Commands::Init => unreachable!(),
            }
        }
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
