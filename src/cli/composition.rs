//! CLI composition dispatch resolves a store only for requests that need one.
use super::Args;
use crate::{
    core::{
        Store, consumer,
        policy::{
            catalog::PolicyCatalog,
            prepare::{self, Attachments},
        },
        retrieval::{RetrieveOptions, RetrievedResource, Retriever},
        store::{AcquiredResource, Acquirer},
    },
    error::{Error, ErrorCode},
    models::{
        claim::Scope,
        policy::{
            PolicyRefs,
            record::CacheMode,
            requests::{Acquisition, Retrieval},
        },
        retrieval::{RetrievedDescriptor, SourceInput},
        store::Destination,
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
    /// Replace source policy references at an expected attachment revision.
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
    retrieval: Retrieval,
    acquisition: Acquisition,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AcquireRetrievedRequest {
    retrieval: MaterialInput,
    acquisition: Acquisition,
}
#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
enum MaterialInput {
    Managed {
        uuid: Uuid,
    },
    Caller {
        request: Box<Retrieval>,
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

fn bind(args: &Args, store: Store) -> Result<Store> {
    match args.as_app {
        Some(app) => Ok(store
            .session(
                app,
                super::credentials::read(args.credential_file.as_deref())?,
            )?
            .into_store()),
        None => Ok(store),
    }
}
fn execution_store(
    args: &Args,
    request: &Retrieval,
    catalog: &PolicyCatalog,
) -> Result<(Option<Store>, CacheMode)> {
    let metadata_needed = matches!(request.source, SourceInput::Registered { .. })
        || request
            .policy_refs
            .ids()
            .iter()
            .chain(request.asset.filter_refs.ids())
            .any(|id| catalog.get(*id).is_err());
    let metadata = if metadata_needed {
        Some(bind(
            args,
            Store::open_metadata(&root(args)?, &crate::core::claim::security::NativeKeyring)?,
        )?)
    } else {
        None
    };
    let mode = if let Some(store) = &metadata {
        store.referenced_cache_mode(request, catalog)?
    } else {
        let empty = PolicyRefs::default();
        prepare::retrieval(
            catalog,
            &Attachments {
                source: &empty,
                application: &empty,
            },
            request,
        )?
        .policies
        .behavior
        .cache
    };
    if matches!(mode, CacheMode::Cached) {
        Ok((Some(bind(args, Store::open(&root(args)?)?)?), mode))
    } else {
        ensure!(
            args.as_app.is_none() || metadata.is_some(),
            Error::new(
                ErrorCode::AccessDenied,
                "application operations require registered sources"
            )
        );
        Ok((metadata, mode))
    }
}
fn retrieve(
    args: &Args,
    request: &Retrieval,
    catalog: &PolicyCatalog,
    scope: Option<&Scope>,
    retained: bool,
) -> Result<RetrievedResource> {
    let (store, mode) = execution_store(args, request, catalog)?;
    ensure!(
        retained || mode != CacheMode::Temporary,
        Error::new(
            ErrorCode::InvalidRequest,
            "temporary retrieval requires run-retrieved or run-acquired"
        )
    );
    match store {
        Some(store) => store.retrieve_referenced_scoped(request, catalog, scope, &options(args)),
        None => Retriever::retrieve_referenced(request, catalog, &options(args)),
    }
}
fn compose(
    args: &Args,
    request: &CompositionRequest,
    catalog: &PolicyCatalog,
    scope: Option<&Scope>,
) -> Result<AcquiredResource> {
    let (store, _) = execution_store(args, &request.retrieval, catalog)?;
    match store {
        Some(store) => store.compose_referenced_scoped(
            &request.retrieval,
            &request.acquisition,
            catalog,
            scope,
            &options(args),
        ),
        None => Acquirer::compose_referenced(
            &request.retrieval,
            &request.acquisition,
            catalog,
            &options(args),
        ),
    }
}
fn material(
    args: &Args,
    input: &MaterialInput,
    catalog: &PolicyCatalog,
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
                bind(
                    args,
                    Store::open_metadata(
                        &root(args)?,
                        &crate::core::claim::security::NativeKeyring,
                    )?,
                )?
                .open_caller_referenced(request, descriptor, catalog, scope)
            } else {
                ensure!(
                    args.as_app.is_none(),
                    Error::new(
                        ErrorCode::AccessDenied,
                        "application material requires a registered source"
                    )
                );
                Retriever::open_caller_referenced(request, descriptor, catalog)
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
    let catalog = match &args.portable_policies {
        Some(file) => PolicyCatalog::new(read::<Vec<crate::models::policy::PolicyRecord>>(file)?)?,
        None => PolicyCatalog::default(),
    };
    let value = match input {
        CompositionCommand::Retrieve {
            file,
            scope: selected_scope,
        } => {
            let mut request: Retrieval = read(file)?;
            let resource = retrieve(
                args,
                &request,
                &catalog,
                scope(selected_scope)?.as_ref(),
                false,
            )?;
            request.source = resource.descriptor().source.clone();
            serde_json::json!({"request": request, "descriptor": resource.descriptor()})
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
            let selected = scope(selected_scope)?;
            let retrieved = material(args, &request.retrieval, &catalog, selected.as_ref())?;
            let result = Acquirer::acquire_or_refresh_referenced(
                retrieved,
                &request.acquisition,
                &catalog,
                selected.as_ref(),
                &options(args),
            )?;
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
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Configuration {
                expected_revision: u64,
                policy_refs: PolicyRefs,
            }
            let configuration: Configuration = read(file)?;
            serde_json::to_value(Store::open(&root(args)?)?.adopt_source_policies(
                *source,
                configuration.expected_revision,
                configuration.policy_refs,
            )?)?
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
            let resource = retrieve(
                args,
                &read(file)?,
                &catalog,
                scope(selected_scope)?.as_ref(),
                true,
            )?;
            let code = consumer::run_retrieved(resource, command(child))?;
            std::process::exit(code);
        }
        CompositionCommand::RunAcquired {
            file,
            scope: selected_scope,
            command: child,
        } => {
            let resource = compose(
                args,
                &read(file)?,
                &catalog,
                scope(selected_scope)?.as_ref(),
            )?;
            let code = consumer::run_acquired(resource, command(child))?;
            std::process::exit(code);
        }
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
