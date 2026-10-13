//! CLI composition dispatch resolves a store only for requests that need one.
use super::Args;
use crate::{
    core::{
        Store, consumer,
        retrieval::{RegisteredRetriever, RetrieveOptions, RetrievedResource, Retriever},
        store::{AcquiredResource, Acquirer},
    },
    error::{Error, ErrorCode},
    models::{
        claim::Scope,
        policy::{Freshness, UpdatePolicy},
        retrieval::{CachePolicy, RetrieveRequest, RetrievedDescriptor, SourceInput},
        store::{AcquisitionRequest, Destination},
    },
};
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};
use uuid::Uuid;

#[derive(clap::Subcommand)]
pub(super) enum CompositionCommand {
    /// Retrieve original material. Temporary results require run-retrieved.
    Retrieve {
        file: PathBuf,
        #[arg(long)]
        scope: Option<String>,
    },
    /// Copy/export/extract an explicitly managed ID or caller-owned receipt.
    AcquireRetrieved {
        file: PathBuf,
        #[arg(long)]
        scope: Option<String>,
    },
    /// Look up a cached source address without refreshing or retaining a lease.
    SourcePath {
        retrieval: Uuid,
        #[arg(long)]
        scope: Option<String>,
    },
    /// Set the default source freshness using a policy JSON file.
    ConfigureSource { source: Uuid, file: PathBuf },
    /// Retire cached retrieval material when no source consumers are live.
    PurgeRetrieval { retrieval: Uuid },
    /// Keep retrieved material alive for a child with literal argument forwarding.
    RunRetrieved {
        file: PathBuf,
        #[arg(long)]
        scope: Option<String>,
        #[arg(last = true, required = true, num_args = 1..)]
        command: Vec<OsString>,
    },
    /// Compose retrieval and acquisition and retain the child's resource lifetime.
    RunAcquired {
        file: PathBuf,
        #[arg(long)]
        scope: Option<String>,
        #[arg(last = true, required = true, num_args = 1..)]
        command: Vec<OsString>,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompositionRequest {
    retrieval: RetrieveRequest,
    acquisition: AcquisitionRequest,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AcquireRetrievedRequest {
    retrieval: MaterialInput,
    acquisition: AcquisitionRequest,
}
#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
enum MaterialInput {
    Managed {
        uuid: Uuid,
    },
    Caller {
        request: Box<RetrieveRequest>,
        descriptor: Box<RetrievedDescriptor>,
    },
}
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}
fn scope(value: &Option<String>) -> Result<Option<Scope>> {
    Ok(value.clone().map(TryInto::try_into).transpose()?)
}
fn root(args: &Args) -> Result<PathBuf> {
    let root = super::root::resolve(args.root.clone())?;
    super::root::require_existing(&root, args.root.is_some())?;
    Ok(root)
}
fn options(args: &Args) -> RetrieveOptions {
    RetrieveOptions {
        test_now: args.test_now,
        test_github_api: args.test_github_api.clone(),
    }
}
fn command(args: &[OsString]) -> Command {
    let mut process = Command::new(&args[0]);
    process.args(&args[1..]);
    process
}
fn retrieve(
    args: &Args,
    request: &RetrieveRequest,
    scope: Option<&Scope>,
) -> Result<RetrievedResource> {
    if matches!(request.cache, CachePolicy::Cached { .. }) {
        let store = Store::open(&root(args)?)?;
        match args.as_app {
            Some(app) => store
                .session(
                    app,
                    super::credentials::read(args.credential_file.as_deref())?,
                )?
                .retrieve(request, scope, &options(args)),
            None => store.retrieve_with(request, &options(args)),
        }
    } else if matches!(request.source, SourceInput::Registered { .. }) {
        let reader = RegisteredRetriever::open(&root(args)?)?;
        match args.as_app {
            Some(app) => reader
                .session(
                    app,
                    super::credentials::read(args.credential_file.as_deref())?,
                )?
                .retrieve(request, scope, &options(args)),
            None => reader.retrieve(request, &options(args)),
        }
    } else {
        ensure!(
            args.as_app.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "application operations require registered sources"
            )
        );
        Retriever::retrieve_with(request, &options(args))
    }
}
fn compose(
    args: &Args,
    request: &CompositionRequest,
    scope: Option<&Scope>,
) -> Result<AcquiredResource> {
    crate::core::policy::composition::validate_composition(
        &request.retrieval,
        &request.acquisition,
        false,
    )?;
    if matches!(request.retrieval.cache, CachePolicy::Cached { .. }) {
        let store = Store::open(&root(args)?)?;
        match args.as_app {
            Some(app) => store
                .session(
                    app,
                    super::credentials::read(args.credential_file.as_deref())?,
                )?
                .acquire_composed(
                    &request.retrieval,
                    &request.acquisition,
                    scope,
                    &options(args),
                ),
            None => store.acquire_composed_with(
                &request.retrieval,
                &request.acquisition,
                &options(args),
            ),
        }
    } else if matches!(request.retrieval.source, SourceInput::Registered { .. }) {
        let reader = RegisteredRetriever::open(&root(args)?)?;
        match args.as_app {
            Some(app) => reader
                .session(
                    app,
                    super::credentials::read(args.credential_file.as_deref())?,
                )?
                .acquire_composed(
                    &request.retrieval,
                    &request.acquisition,
                    scope,
                    &options(args),
                ),
            None => {
                reader.acquire_composed(&request.retrieval, &request.acquisition, &options(args))
            }
        }
    } else {
        ensure!(
            args.as_app.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "application operations require registered sources"
            )
        );
        Acquirer::retrieve_and_acquire_with(
            &request.retrieval,
            &request.acquisition,
            &options(args),
        )
    }
}
fn material(
    args: &Args,
    input: &MaterialInput,
    scope: Option<&Scope>,
) -> Result<RetrievedResource> {
    match input {
        MaterialInput::Managed { uuid } => {
            let store = Store::open(&root(args)?)?;
            match args.as_app {
                Some(app) => store
                    .session(
                        app,
                        super::credentials::read(args.credential_file.as_deref())?,
                    )?
                    .lease_retrieval(*uuid, scope),
                None => store.lease_retrieval(*uuid),
            }
        }
        MaterialInput::Caller {
            request,
            descriptor,
        } => {
            if matches!(request.source, SourceInput::Registered { .. }) {
                let reader = RegisteredRetriever::open(&root(args)?)?;
                match args.as_app {
                    Some(app) => reader
                        .session(
                            app,
                            super::credentials::read(args.credential_file.as_deref())?,
                        )?
                        .open_caller(request, descriptor, scope),
                    None => reader.open_caller(request, descriptor),
                }
            } else {
                ensure!(
                    args.as_app.is_none(),
                    Error::new(
                        ErrorCode::AccessDenied,
                        "application material requires a registered source"
                    )
                );
                Retriever::open_caller(request, descriptor)
            }
        }
    }
}

pub(super) fn run(input: &CompositionCommand, args: &Args) -> Result<()> {
    ensure!(
        !args.force_refresh && !args.allow_stale,
        Error::new(
            ErrorCode::InvalidRequest,
            "composition commands use typed freshness/update/stale policies in their requests"
        )
    );
    ensure!(
        args.credential_file.is_none() || args.as_app.is_some(),
        Error::new(
            ErrorCode::InvalidRequest,
            "credential-file requires as-app for composition commands"
        )
    );
    let value = match input {
        CompositionCommand::Retrieve {
            file,
            scope: selected_scope,
        } => {
            let request: RetrieveRequest = read(file)?;
            ensure!(
                !matches!(request.cache, CachePolicy::Temporary {}),
                Error::new(
                    ErrorCode::InvalidRequest,
                    "temporary retrieval requires run-retrieved or run-acquired"
                )
            );
            let resource = retrieve(args, &request, scope(selected_scope)?.as_ref())?;
            serde_json::json!({"request": resource.request(), "descriptor": resource.descriptor()})
        }
        CompositionCommand::AcquireRetrieved {
            file,
            scope: selected_scope,
        } => {
            let request: AcquireRetrievedRequest = read(file)?;
            ensure!(
                !matches!(request.acquisition.destination, Destination::Temporary {}),
                Error::new(
                    ErrorCode::InvalidRequest,
                    "temporary acquisition requires run-acquired"
                )
            );
            let retrieved = material(args, &request.retrieval, scope(selected_scope)?.as_ref())?;
            let result = if matches!(request.acquisition.update_policy, UpdatePolicy::Manual {}) {
                Acquirer::acquire(&retrieved, &request.acquisition)?
            } else {
                Acquirer::refresh_with(retrieved, &request.acquisition, &options(args))?
            };
            serde_json::to_value(result.descriptor())?
        }
        CompositionCommand::SourcePath {
            retrieval,
            scope: selected_scope,
        } => {
            let store = Store::open(&root(args)?)?;
            let path = match args.as_app {
                Some(app) => store
                    .session(
                        app,
                        super::credentials::read(args.credential_file.as_deref())?,
                    )?
                    .source_path(*retrieval, scope(selected_scope)?.as_ref())?,
                None => store.source_path(*retrieval)?,
            };
            serde_json::json!({"path": path})
        }
        CompositionCommand::ConfigureSource { source, file } => {
            ensure!(
                args.as_app.is_none(),
                Error::new(
                    ErrorCode::AccessDenied,
                    "source configuration requires administration"
                )
            );
            let policy: Freshness = read(file)?;
            serde_json::to_value(
                Store::open(&root(args)?)?.configure_source_freshness(*source, policy)?,
            )?
        }
        CompositionCommand::PurgeRetrieval { retrieval } => {
            ensure!(
                args.as_app.is_none(),
                Error::new(
                    ErrorCode::AccessDenied,
                    "retrieval purge requires administration"
                )
            );
            serde_json::json!({"purged": Store::open(&root(args)?)?.purge_retrieval(*retrieval)?})
        }
        CompositionCommand::RunRetrieved {
            file,
            scope: selected_scope,
            command: child,
        } => {
            let resource = retrieve(args, &read(file)?, scope(selected_scope)?.as_ref())?;
            let code = consumer::run_retrieved(resource, command(child))?;
            std::process::exit(code);
        }
        CompositionCommand::RunAcquired {
            file,
            scope: selected_scope,
            command: child,
        } => {
            let resource = compose(args, &read(file)?, scope(selected_scope)?.as_ref())?;
            let code = consumer::run_acquired(resource, command(child))?;
            std::process::exit(code);
        }
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
