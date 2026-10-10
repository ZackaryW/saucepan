use crate::models::store::VerificationMode;
use crate::{core::Store, models::source::Source};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
mod credentials;
mod root;
mod source_query;

#[derive(Clone, Copy, clap::ValueEnum)]
enum ErrorFormat {
    Text,
    Json,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum SourceVisibility {
    Public,
    Private,
}

#[derive(Parser)]
#[command(version = concat!(env!("CARGO_PKG_VERSION"),"+",env!("SAUCEPAN_BUILD_ID")))]
struct Args {
    #[arg(long, global = true, value_enum, default_value = "text")]
    error_format: ErrorFormat,
    /// Read a 32-byte application credential from this file.
    #[arg(long, global = true)]
    credential_file: Option<PathBuf>,
    /// Use this application's policies for queries and resource operations.
    #[arg(long, global = true)]
    as_app: Option<uuid::Uuid>,
    /// Store directory; defaults to .saucepan in the user's home directory.
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
    /// Register by explicit key using a JSON application registration.
    RegisterApplication {
        file: PathBuf,
        #[arg(long)]
        credential_out: Option<PathBuf>,
    },
    /// Trusted local administration with expected_revision in the JSON payload.
    ConfigureApplication {
        app: uuid::Uuid,
        file: PathBuf,
        #[arg(long)]
        credential_out: Option<PathBuf>,
    },
    EnsureClaim {
        app: uuid::Uuid,
        file: PathBuf,
    },
    RecoverClaim {
        app: uuid::Uuid,
        claim: uuid::Uuid,
    },
    DiscoverClaims {
        file: PathBuf,
    },
    /// Lease a claim or contained view for a child process.
    RunClaim {
        app: uuid::Uuid,
        claim: uuid::Uuid,
        #[arg(long)]
        relative: Option<String>,
        #[arg(long, value_enum, default_value = "metadata")]
        verification: VerificationMode,
        #[arg(last = true, required = true, num_args = 1..)]
        command: Vec<std::ffi::OsString>,
    },
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
        #[arg(long)]
        scope: String,
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
    /// Find registered sources; no app UUID or query file is required.
    #[command(
        after_help = "Examples:\n  saucepan query-sources\n  saucepan query-sources --type github_release --repo owner/project\n  saucepan --as-app APP_UUID query-sources\n  saucepan query-sources query.json\n  saucepan --root ./scratch query-sources\n\nWithout --as-app only explicitly public sources appear. An empty result can mean\nno sources have been published. Run saucepan init if the default store is absent."
    )]
    QuerySources(source_query::SourceQueryArgs),
    /// Trusted administration: publish or withdraw a source definition.
    SetSourceVisibility {
        source: uuid::Uuid,
        #[arg(value_enum)]
        visibility: SourceVisibility,
    },
    Inspect {
        uuid: uuid::Uuid,
    },
    Acquire {
        file: PathBuf,
        #[arg(long)]
        app: Option<uuid::Uuid>,
        /// JSON object of string labels, published with the new claim.
        #[arg(long, requires = "app")]
        context: Option<PathBuf>,
        #[arg(long, requires = "app")]
        scope: Option<String>,
    },
    /// Replace a claim's complete context at an observed application revision.
    SetClaimContext {
        app: uuid::Uuid,
        claim: uuid::Uuid,
        file: PathBuf,
        #[arg(long)]
        expected_revision: u64,
    },
    /// Search existing claims using a JSON object containing app and where.
    QueryClaims {
        file: PathBuf,
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
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) if !error.use_stderr() => {
            error.print()?;
            return Ok(());
        }
        Err(_) => {
            return Err(crate::error::Error::new(
                crate::error::ErrorCode::InvalidRequest,
                "invalid command arguments; see --help",
            )
            .into());
        }
    };
    let explicit_root = args.root.is_some();
    let root = root::resolve(args.root)?;
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
            root::require_existing(&root, explicit_root)?;
            let mut store = Store::open(&root)?;
            let app = command.application();
            anyhow::ensure!(
                args.as_app.is_none() || app.is_none() || args.as_app == app,
                crate::error::Error::new(
                    crate::error::ErrorCode::AccessDenied,
                    "application identity mismatch"
                )
            );
            let bound = args.as_app.or(app);
            if let Some(app) = bound {
                anyhow::ensure!(
                    !command.administration(),
                    crate::error::Error::new(
                        crate::error::ErrorCode::AccessDenied,
                        "command requires trusted administration"
                    )
                );
                store = store
                    .session(app, credentials::read(args.credential_file.as_deref())?)?
                    .into_store();
            }
            match command {
                Commands::RegisterApplication {
                    file,
                    credential_out,
                } => {
                    let input = serde_json::from_slice(&std::fs::read(file)?)?;
                    let proof = credentials::read(args.credential_file.as_deref())?;
                    let result =
                        store.register_application_delivering(&input, proof.as_ref(), |token| {
                            credentials::deliver(credential_out.as_deref(), token)
                        })?;
                    serde_json::to_value(result.application)?
                }
                Commands::ConfigureApplication {
                    app,
                    file,
                    credential_out,
                } => {
                    let input = serde_json::from_slice(&std::fs::read(file)?)?;
                    let proof = credentials::read(args.credential_file.as_deref())?;
                    store.configure_application_delivering(
                        app,
                        input,
                        proof.as_ref(),
                        |token| credentials::deliver(credential_out.as_deref(), token),
                    )?;
                    serde_json::to_value(store.application(app)?)?
                }
                Commands::EnsureClaim { app, file } => serde_json::to_value(store.ensure_claim(
                    app,
                    &serde_json::from_slice(&std::fs::read(file)?)?,
                    &options,
                )?)?,
                Commands::RecoverClaim { app, claim } => {
                    serde_json::to_value(store.recover_claim(app, claim, &options)?)?
                }
                Commands::DiscoverClaims { file } => serde_json::to_value(
                    store.discover_claims(&serde_json::from_slice(&std::fs::read(file)?)?)?,
                )?,
                Commands::RunClaim {
                    app,
                    claim,
                    relative,
                    verification,
                    command,
                } => {
                    let view = crate::models::claim::CoveredView {
                        app,
                        claim,
                        entry: uuid::Uuid::nil(),
                        relative,
                    };
                    let lease = store.lease_view(&view, verification)?;
                    let mut process = std::process::Command::new(&command[0]);
                    process
                        .args(&command[1..])
                        .env("SAUCEPAN_RESOURCE_DIR", lease.directory());
                    std::process::exit(crate::core::launch::launch_leased(process, lease)?);
                }
                Commands::Operations { app } => {
                    if let Some(identity) = &store.session {
                        store.authorize_application_action(
                            identity.app,
                            crate::models::claim::policy::Action::Read,
                        )?;
                        let registry = store.registry(identity.app)?;
                        let visible: Vec<_> = store
                            .operations(Some(identity.app))?
                            .into_iter()
                            .filter(|operation| {
                                operation.desired_registry.as_ref().is_none_or(|desired| {
                                    desired.claims.values().all(|claim| {
                                        store
                                            .authorize_claim(
                                                &registry,
                                                claim,
                                                crate::models::claim::policy::Action::Read,
                                            )
                                            .is_ok()
                                    })
                                })
                            })
                            .collect();
                        serde_json::to_value(visible)?
                    } else {
                        serde_json::to_value(store.operations(app)?)?
                    }
                }
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
                    scope,
                    name,
                    request,
                    probe,
                    built_shim,
                    shim,
                    operation,
                } => operation_value(
                    store.install_application(
                        &crate::models::index::lifecycle::Installation {
                            scope: scope.try_into()?,
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
                Commands::QuerySources(input) => {
                    serde_json::to_value(store.query_sources(&input.into_query()?)?)?
                }
                Commands::SetSourceVisibility { source, visibility } => {
                    serde_json::to_value(store.set_source_visibility(
                        source,
                        matches!(visibility, SourceVisibility::Public),
                    )?)?
                }
                Commands::Inspect { uuid } => serde_json::to_value(store.record(uuid)?)?,
                Commands::Acquire {
                    file,
                    app,
                    context,
                    scope,
                } => {
                    let request = serde_json::from_slice(&std::fs::read(file)?)?;
                    match app {
                        Some(app) => {
                            let context = match context {
                                Some(file) => serde_json::from_slice(&std::fs::read(file)?)?,
                                None => Default::default(),
                            };
                            serde_json::to_value(
                                store.acquire_for_with_context(
                                    app,
                                    &scope
                                        .context(crate::error::Error::new(
                                            crate::error::ErrorCode::InvalidRequest,
                                            "--scope is required with --app",
                                        ))?
                                        .try_into()?,
                                    &request,
                                    &context,
                                    &options,
                                )?,
                            )?
                        }
                        None => serde_json::to_value(store.acquire_with(&request, &options)?)?,
                    }
                }
                Commands::SetClaimContext {
                    app,
                    claim,
                    file,
                    expected_revision,
                } => {
                    let context = serde_json::from_slice(&std::fs::read(file)?)?;
                    serde_json::to_value(store.set_claim_context(
                        app,
                        claim,
                        &context,
                        expected_revision,
                    )?)?
                }
                Commands::QueryClaims { file } => {
                    let query: crate::core::query::ClaimQuery =
                        serde_json::from_slice(&std::fs::read(file)?)?;
                    if store.session.is_none() {
                        store = store
                            .session(
                                query.app,
                                credentials::read(args.credential_file.as_deref())?,
                            )?
                            .into_store();
                    }
                    serde_json::to_value(store.query_claims(&query)?)?
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

impl Commands {
    fn application(&self) -> Option<uuid::Uuid> {
        match self {
            Self::EnsureClaim { app, .. }
            | Self::RecoverClaim { app, .. }
            | Self::RunClaim { app, .. }
            | Self::SelfUpdate { app, .. }
            | Self::UpdateApp { app, .. }
            | Self::Rollback { app, .. }
            | Self::SetClaimContext { app, .. }
            | Self::UpdateClaim { app, .. }
            | Self::ReleaseClaim { app, .. }
            | Self::Claims { app }
            | Self::Activate { app, .. }
            | Self::Deactivate { app }
            | Self::Uninstall { app, .. }
            | Self::InstallShim { app, .. }
            | Self::ReplaceShim { app, .. }
            | Self::AdoptShim { app, .. } => Some(*app),
            Self::Acquire { app, .. } | Self::Operations { app } => *app,
            _ => None,
        }
    }
    fn administration(&self) -> bool {
        matches!(
            self,
            Self::RegisterApplication { .. }
                | Self::ConfigureApplication { .. }
                | Self::RegisterApp { .. }
                | Self::RegisterSource { .. }
                | Self::SetSourceVisibility { .. }
                | Self::Install { .. }
                | Self::Bootstrap { .. }
                | Self::Reconcile { .. }
                | Self::Reclaim
                | Self::Settings { .. }
                | Self::Inspect { .. }
                | Self::Reacquire { .. }
                | Self::Acquire { app: None, .. }
        )
    }
}

/// Parse only our option prefix; never reinterpret a child's arguments after --.
pub fn report_error(error: &anyhow::Error) {
    let mut json = false;
    let mut args = std::env::args_os().skip(1).take_while(|arg| arg != "--");
    while let Some(arg) = args.next() {
        if arg == "--error-format" {
            json = args.next().is_some_and(|value| value == "json");
        } else if arg == "--error-format=json" {
            json = true;
        } else if arg == "--error-format=text" {
            json = false;
        }
    }
    let safe = crate::error::Error::from_anyhow(error);
    if json {
        eprintln!("{}", serde_json::json!({"error": safe}));
    } else {
        eprintln!("{safe}");
    }
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
