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
