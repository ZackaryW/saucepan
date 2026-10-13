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
        retrieval::SourceInput,
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
    pub fn open_caller_referenced(
        &self,
        input: &crate::models::policy::requests::Retrieval,
        descriptor: &crate::models::retrieval::RetrievedDescriptor,
        catalog: &crate::core::policy::catalog::PolicyCatalog,
    ) -> Result<RetrievedResource> {
        self.store
            .open_caller_referenced(input, descriptor, catalog, None)
    }

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
    pub fn open_caller_referenced(
        &self,
        input: &crate::models::policy::requests::Retrieval,
        descriptor: &crate::models::retrieval::RetrievedDescriptor,
        catalog: &crate::core::policy::catalog::PolicyCatalog,
        scope: Option<&Scope>,
    ) -> Result<RetrievedResource> {
        self.session
            .open_caller_referenced(input, descriptor, catalog, scope)
    }

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
}

impl RegisteredRetriever {
    pub fn retrieve(
        &self,
        input: &crate::models::policy::requests::Retrieval,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        self.retrieve_referenced(input, &Default::default(), options)
    }
    pub fn open_caller(
        &self,
        input: &crate::models::policy::requests::Retrieval,
        descriptor: &crate::models::retrieval::RetrievedDescriptor,
    ) -> Result<RetrievedResource> {
        self.store
            .open_caller_referenced(input, descriptor, &Default::default(), None)
    }
}
impl RegisteredAppRetriever {
    pub fn retrieve(
        &self,
        input: &crate::models::policy::requests::Retrieval,
        scope: Option<&Scope>,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        self.retrieve_referenced(input, &Default::default(), scope, options)
    }
}
