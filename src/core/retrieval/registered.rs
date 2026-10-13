//! Metadata-only registered-source access. Opening this reader never recovers.
use super::{RetrieveOptions, RetrievedResource};
use crate::{
    core::{
        Store,
        claim::{
            security::{KeyProvider, NativeKeyring},
            session::AppSession,
        },
    },
    error::{Error, ErrorCode},
    models::{
        claim::{Scope, security::Credential},
        retrieval::{CachePolicy, RetrieveRequest, SourceInput},
    },
};
use anyhow::{Result, ensure};
use std::path::Path;
use uuid::Uuid;

pub struct RegisteredRetriever {
    store: Store,
}
pub struct RegisteredAppRetriever {
    session: AppSession,
}
impl RegisteredRetriever {
    pub fn retrieve_referenced(
        &self,
        input: &crate::models::policy::requests::Retrieval,
        catalog: &crate::core::policy::catalog::PolicyCatalog,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        ensure!(
            matches!(input.source, SourceInput::Registered { .. }),
            Error::new(
                ErrorCode::InvalidRequest,
                "registered reader requires a registered source"
            )
        );
        self.store.retrieve_referenced(input, catalog, options)
    }
    pub fn compose_referenced(
        &self,
        input: &crate::models::policy::requests::Retrieval,
        acquisition: &crate::models::policy::requests::Acquisition,
        catalog: &crate::core::policy::catalog::PolicyCatalog,
        options: &RetrieveOptions,
    ) -> Result<crate::core::store::AcquiredResource> {
        ensure!(
            matches!(input.source, SourceInput::Registered { .. }),
            Error::new(
                ErrorCode::InvalidRequest,
                "registered reader requires a registered source"
            )
        );
        self.store
            .compose_referenced(input, acquisition, catalog, options)
    }
    pub fn open(root: &Path) -> Result<Self> {
        Self::open_with_keys(root, &NativeKeyring)
    }
    pub fn open_with_keys(root: &Path, keys: &dyn KeyProvider) -> Result<Self> {
        Ok(Self {
            store: Store::open_metadata(root, keys)?,
        })
    }
    pub fn retrieve(
        &self,
        request: &RetrieveRequest,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        validate(&self.store, request)?;
        self.store.retrieve_authorized(request, options, None)
    }
    pub fn session(
        &self,
        app: Uuid,
        credential: Option<Credential>,
    ) -> Result<RegisteredAppRetriever> {
        Ok(RegisteredAppRetriever {
            session: self.store.session(app, credential)?,
        })
    }
}
impl RegisteredAppRetriever {
    pub fn retrieve_referenced(
        &self,
        input: &crate::models::policy::requests::Retrieval,
        catalog: &crate::core::policy::catalog::PolicyCatalog,
        scope: Option<&Scope>,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        self.session
            .retrieve_referenced(input, catalog, scope, options)
    }
    pub fn compose_referenced(
        &self,
        input: &crate::models::policy::requests::Retrieval,
        acquisition: &crate::models::policy::requests::Acquisition,
        catalog: &crate::core::policy::catalog::PolicyCatalog,
        scope: Option<&Scope>,
        options: &RetrieveOptions,
    ) -> Result<crate::core::store::AcquiredResource> {
        self.session
            .compose_referenced(input, acquisition, catalog, scope, options)
    }
    pub fn retrieve(
        &self,
        request: &RetrieveRequest,
        scope: Option<&Scope>,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        validate(&self.session.store, request)?;
        self.session.retrieve(request, scope, options)
    }
}
fn validate(store: &Store, request: &RetrieveRequest) -> Result<()> {
    ensure!(
        matches!(request.source, SourceInput::Registered { .. })
            && !matches!(request.cache, CachePolicy::Cached { .. }),
        Error::new(
            ErrorCode::InvalidRequest,
            "registered fresh reader requires registered temporary/no_store retrieval"
        )
    );
    if let CachePolicy::NoStore { destination } = &request.cache {
        crate::utils::path::disjoint_destination(store.root(), destination)?;
    }
    Ok(())
}

impl RegisteredRetriever {
    pub fn open_caller(
        &self,
        request: &RetrieveRequest,
        descriptor: &crate::models::retrieval::RetrievedDescriptor,
    ) -> Result<RetrievedResource> {
        self.store.open_caller_authorized(request, descriptor, None)
    }
}
impl RegisteredAppRetriever {
    pub fn open_caller(
        &self,
        request: &RetrieveRequest,
        descriptor: &crate::models::retrieval::RetrievedDescriptor,
        scope: Option<&Scope>,
    ) -> Result<RetrievedResource> {
        let SourceInput::Registered { source_uuid } = request.source else {
            return Err(Error::new(
                ErrorCode::AccessDenied,
                "application material requires a registered source",
            )
            .into());
        };
        let authorization =
            self.session
                .store
                .begin_composition(self.session.app, scope, source_uuid, None)?;
        self.session
            .store
            .open_caller_authorized(request, descriptor, Some(authorization))
    }
}

impl RegisteredRetriever {
    pub fn acquire_composed(
        &self,
        retrieval: &RetrieveRequest,
        acquisition: &crate::models::store::AcquisitionRequest,
        options: &RetrieveOptions,
    ) -> Result<crate::core::store::AcquiredResource> {
        validate(&self.store, retrieval)?;
        self.store
            .acquire_composed_authorized(retrieval, acquisition, options, None)
    }
}
impl RegisteredAppRetriever {
    pub fn acquire_composed(
        &self,
        retrieval: &RetrieveRequest,
        acquisition: &crate::models::store::AcquisitionRequest,
        scope: Option<&Scope>,
        options: &RetrieveOptions,
    ) -> Result<crate::core::store::AcquiredResource> {
        validate(&self.session.store, retrieval)?;
        self.session
            .acquire_composed(retrieval, acquisition, scope, options)
    }
}
