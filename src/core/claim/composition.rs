//! Authorization context follows an operation even when no claim is requested.
use super::session::AppSession;
use crate::{
    core::{
        Store,
        retrieval::{RetrieveOptions, RetrievedResource, Retriever},
        store::AcquiredResource,
    },
    error::{Error, ErrorCode},
    models::{
        claim::{
            Scope,
            policy::{Action, Overrides},
        },
        retrieval::{CachePolicy, RetrieveRequest, SourceInput},
        store::{AcquisitionRequest, acquisition::AcquisitionOwner},
    },
};
use anyhow::{Result, ensure};
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct Authorization {
    pub owner: AcquisitionOwner,
    pub revision: u64,
    pub update: bool,
}
impl Authorization {
    pub(crate) fn check(
        &self,
        store: &Store,
        source: Uuid,
        acquisition: Option<&AcquisitionRequest>,
    ) -> Result<()> {
        let application = store.application(self.owner.app)?;
        let registry = store.registry(self.owner.app)?;
        store.authorize_resource(
            &registry,
            Action::Acquire,
            Some(source),
            self.owner.scope.as_ref(),
        )?;
        if self.update {
            store.authorize_resource(
                &registry,
                Action::Update,
                Some(source),
                self.owner.scope.as_ref(),
            )?;
        }
        ensure!(
            application.revision == self.revision,
            Error::new(
                ErrorCode::RevisionConflict,
                "application changed during composition"
            )
        );
        if let Some(acquisition) = acquisition {
            let revision = crate::core::policy::composition::effective_revision(acquisition)?;
            let effective = registry.registry.policies.evaluate(&Overrides {
                identical: Some(acquisition.identical),
                keep: revision.map(|policy| policy.keep()).transpose()?,
                useable_app: Some(acquisition.protection.is_some()),
                update_policy: Some(acquisition.update_policy),
                ..Default::default()
            })?;
            ensure!(
                effective.output.protection == acquisition.protection,
                Error::new(
                    ErrorCode::PolicyConflict,
                    "registered protection requires a managed output"
                )
            );
        }
        Ok(())
    }
}
impl Store {
    pub(crate) fn begin_composition(
        &self,
        app: Uuid,
        scope: Option<&Scope>,
        source: Uuid,
        acquisition: Option<&AcquisitionRequest>,
    ) -> Result<Authorization> {
        let _lock = self.lock()?;
        let authorization = Authorization {
            owner: AcquisitionOwner {
                app,
                scope: scope.cloned(),
                claim: None,
            },
            revision: self.application(app)?.revision,
            update: false,
        };
        authorization.check(self, source, acquisition)?;
        Ok(authorization)
    }

    pub(crate) fn retrieve_authorized(
        &self,
        request: &RetrieveRequest,
        options: &RetrieveOptions,
        authorization: Option<Authorization>,
    ) -> Result<RetrievedResource> {
        if !matches!(request.cache, CachePolicy::Cached { .. }) && !self.metadata_only {
            let mut reader = self.clone();
            reader.metadata_only = true;
            return reader.retrieve_authorized(request, options, authorization);
        }
        if let CachePolicy::NoStore { destination } = &request.cache {
            crate::utils::path::disjoint_destination(self.root(), destination)?;
        }
        if self.session.is_some() {
            ensure!(
                authorization.is_some(),
                Error::new(
                    ErrorCode::AccessDenied,
                    "missing operation scope authorization"
                )
            );
        }
        let source = match request.source {
            SourceInput::Registered { source_uuid } => Some(source_uuid),
            SourceInput::Inline { .. } => None,
        };
        if let Some(authorization) = &authorization {
            authorization.check(
                self,
                source.ok_or_else(|| {
                    Error::new(
                        ErrorCode::AccessDenied,
                        "application operations require a registered source",
                    )
                })?,
                None,
            )?;
        }
        let mut resource = if matches!(request.cache, CachePolicy::Cached { .. }) {
            self.retrieve_prevalidated(request, options)?
        } else {
            let mut inline = request.clone();
            if let Some(source) = source {
                inline.source = SourceInput::Inline {
                    source: self.source(source)?.source,
                };
            }
            let mut resource = Retriever::retrieve_finishing(&inline, options, None, || {
                let guard = self.lock()?;
                if let Some(authorization) = &authorization {
                    authorization.check(self, source.unwrap(), None)?;
                }
                Ok(guard)
            })?;
            resource.request = request.clone();
            resource.descriptor.source = request.source.clone();
            resource.store = Some(self.clone());
            resource
        };
        if matches!(request.cache, CachePolicy::Cached { .. })
            && let Some(authorization) = &authorization
        {
            authorization.check(self, source.unwrap(), None)?;
        }
        resource.authorization = authorization;
        Ok(resource)
    }
}
impl AppSession {
    pub fn retrieve(
        &self,
        request: &RetrieveRequest,
        scope: Option<&Scope>,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        let SourceInput::Registered { source_uuid } = request.source else {
            return Err(Error::new(
                ErrorCode::AccessDenied,
                "application retrieval requires a registered source",
            )
            .into());
        };
        let mut store = self.store.clone();
        store.metadata_only |= !matches!(request.cache, CachePolicy::Cached { .. });
        let authorization = store.begin_composition(self.app, scope, source_uuid, None)?;
        store.retrieve_authorized(request, options, Some(authorization))
    }

    pub fn acquire_composed(
        &self,
        retrieval: &RetrieveRequest,
        acquisition: &AcquisitionRequest,
        scope: Option<&Scope>,
        options: &RetrieveOptions,
    ) -> Result<AcquiredResource> {
        let SourceInput::Registered { source_uuid } = retrieval.source else {
            return Err(Error::new(
                ErrorCode::AccessDenied,
                "application acquisition requires a registered source",
            )
            .into());
        };
        let mut store = self.store.clone();
        store.metadata_only |= !matches!(retrieval.cache, CachePolicy::Cached { .. });
        let authorization =
            store.begin_composition(self.app, scope, source_uuid, Some(acquisition))?;
        store.acquire_composed_authorized(retrieval, acquisition, options, Some(authorization))
    }
}

impl AppSession {
    pub fn lease_retrieval(&self, id: Uuid, scope: Option<&Scope>) -> Result<RetrievedResource> {
        let record = self.store.retrieval_record(id)?;
        let authorization =
            self.store
                .begin_composition(self.app, scope, record.source_uuid, None)?;
        self.store
            .lease_retrieval_authorized(id, Some(authorization))
    }
    pub fn source_path(&self, id: Uuid, scope: Option<&Scope>) -> Result<std::path::PathBuf> {
        let _lock = self.store.lock()?;
        self.store.application(self.app)?;
        let record = self.store.retrieval_record(id)?;
        self.store.authorize_resource(
            &self.store.registry(self.app)?,
            Action::Read,
            Some(record.source_uuid),
            scope,
        )?;
        self.store.source_path(id)
    }
}

impl Store {
    pub(crate) fn open_caller_authorized(
        &self,
        request: &RetrieveRequest,
        descriptor: &crate::models::retrieval::RetrievedDescriptor,
        authorization: Option<Authorization>,
    ) -> Result<RetrievedResource> {
        ensure!(
            self.metadata_only,
            "caller material reads require metadata-only access"
        );
        let SourceInput::Registered { source_uuid } = request.source else {
            return Err(Error::new(ErrorCode::InvalidRequest, "registered source required").into());
        };
        if let Some(authorization) = &authorization {
            authorization.check(self, source_uuid, None)?;
        }
        let _lock = self.lock()?;
        let source = self.source(source_uuid)?.source;
        ensure!(
            descriptor.source == request.source,
            Error::new(ErrorCode::InvalidRequest, "caller source identity mismatch")
        );
        let mut inline = request.clone();
        inline.source = SourceInput::Inline { source };
        let mut supplied = descriptor.clone();
        supplied.source = inline.source.clone();
        let mut result = Retriever::open_caller(&inline, &supplied)?;
        if let Some(authorization) = &authorization {
            authorization.check(self, source_uuid, None)?;
        }
        result.request = request.clone();
        result.descriptor.source = request.source.clone();
        result.store = Some(self.clone());
        result.authorization = authorization;
        Ok(result)
    }
}
