//! Reference-based entry points delegate material work to the existing engines.
use super::{
    catalog::PolicyCatalog,
    fence::PolicyFence,
    prepare::{self, Attachments},
    resolve,
};
use crate::{
    core::{
        Store,
        claim::session::AppSession,
        retrieval::{RetrieveOptions, RetrievedResource, Retriever},
        store::{AcquiredResource, Acquirer},
    },
    error::{Error, ErrorCode},
    models::{
        claim::{Scope, policy::Action},
        policy::{
            PolicyRefs,
            requests::{Acquisition, Retrieval},
        },
        retrieval::{CachePolicy, SourceInput},
        store::Destination,
    },
};
use anyhow::{Result, ensure};

impl Retriever {
    /// Inline fresh operations use only this caller-owned catalog. They never
    /// resolve a home directory or initialize a store/keyring.
    pub fn retrieve_referenced(
        input: &Retrieval,
        catalog: &PolicyCatalog,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        let empty = PolicyRefs::default();
        let plan = prepare::retrieval(
            catalog,
            &Attachments {
                source: &empty,
                application: &empty,
            },
            input,
        )?;
        Self::retrieve_with(&plan.request, options)
    }
}
impl Acquirer {
    /// Copy a retained source handle without another fetch. Automatic updates
    /// belong to compose_referenced, which owns and can replace that handle.
    pub fn acquire_referenced(
        retrieved: &RetrievedResource,
        input: &Acquisition,
        portable: &PolicyCatalog,
        scope: Option<&Scope>,
    ) -> Result<AcquiredResource> {
        let empty = PolicyRefs::default();
        let request = if let Some(store) = &retrieved.store {
            let source_input = Retrieval {
                source: retrieved.request.source.clone(),
                selector: retrieved.request.selector.clone(),
                asset: Default::default(),
                policy_refs: Default::default(),
                destination: None,
                expected_sha256: None,
            };
            let context = Context::new(store, &source_input, portable)?;
            let plan = prepare::acquisition(&context.catalog, &context.attachments(), input)?;
            if matches!(input.destination, Destination::Managed {}) {
                context.durable(&source_input, Some(input))?;
            }
            context.authorize(&plan.policies, &source_input, scope, Action::Acquire)?;
            store.check_policy_fence()?;
            plan.request
        } else {
            prepare::acquisition(
                portable,
                &Attachments {
                    source: &empty,
                    application: &empty,
                },
                input,
            )?
            .request
        };
        Self::acquire(retrieved, &request)
    }
    pub fn compose_referenced(
        retrieval: &Retrieval,
        acquisition: &Acquisition,
        catalog: &PolicyCatalog,
        options: &RetrieveOptions,
    ) -> Result<AcquiredResource> {
        let empty = PolicyRefs::default();
        let attachments = Attachments {
            source: &empty,
            application: &empty,
        };
        let retrieval = prepare::retrieval(catalog, &attachments, retrieval)?;
        let acquisition = prepare::acquisition(catalog, &attachments, acquisition)?;
        Self::retrieve_and_acquire_with(&retrieval.request, &acquisition.request, options)
    }
}

struct Context {
    store: Store,
    registered: PolicyCatalog,
    catalog: PolicyCatalog,
    source: PolicyRefs,
    application: PolicyRefs,
}
impl Context {
    fn new(store: &Store, input: &Retrieval, portable: &PolicyCatalog) -> Result<Self> {
        // Preflight is observational even if the caller opened a normal Store.
        let mut reader = store.clone();
        reader.metadata_only = true;
        let _lock = reader.lock()?;
        let snapshot = reader.metadata.snapshot()?;
        let registered = super::persistence::load(&reader.metadata, &snapshot)?;
        let catalog = registered.with_portable(portable)?;
        let source_id = match input.source {
            SourceInput::Registered { source_uuid } => Some(source_uuid),
            _ => None,
        };
        let app_id = reader.session.as_ref().map(|session| session.app);
        let source = source_id
            .map(|id| reader.source(id).map(|source| source.policy_refs))
            .transpose()?
            .unwrap_or_default();
        let application = app_id
            .map(|id| reader.registry(id).map(|r| r.registry.policy_refs))
            .transpose()?
            .unwrap_or_default();
        let mut execution = store.clone();
        execution.policy_fence = Some(PolicyFence::capture(&reader, source_id, app_id)?);
        Ok(Self {
            store: execution,
            registered,
            catalog,
            source,
            application,
        })
    }
    fn attachments(&self) -> Attachments<'_> {
        Attachments {
            source: &self.source,
            application: &self.application,
        }
    }
    fn durable(&self, input: &Retrieval, acquisition: Option<&Acquisition>) -> Result<()> {
        prepare::require_registered(
            &self.registered,
            &self.catalog,
            input
                .policy_refs
                .ids()
                .iter()
                .chain(input.asset.filter_refs.ids())
                .chain(acquisition.into_iter().flat_map(|a| a.policy_refs.ids()))
                .copied(),
        )
    }
    fn authorize(
        &self,
        plan: &crate::models::policy::effective::EvaluatedPolicies,
        input: &Retrieval,
        scope: Option<&Scope>,
        action: Action,
    ) -> Result<()> {
        // Registered source restrictions also apply to unclaimed retrieval.
        let source = match input.source {
            SourceInput::Registered { source_uuid } => Some(source_uuid),
            _ => None,
        };
        resolve::authorize(plan, action, source, scope)?;
        if let Some(identity) = &self.store.session {
            self.store.authorize_resource(
                &self.store.registry(identity.app)?,
                action,
                source,
                scope,
            )?;
        }
        self.store.check_policy_fence()
    }
}

impl Store {
    pub fn retrieve_referenced(
        &self,
        input: &Retrieval,
        portable: &PolicyCatalog,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        ensure!(
            self.session.is_none(),
            Error::new(ErrorCode::AccessDenied, "use scoped application retrieval")
        );
        self.retrieve_referenced_scoped(input, portable, None, options)
    }
    pub(crate) fn retrieve_referenced_scoped(
        &self,
        input: &Retrieval,
        portable: &PolicyCatalog,
        scope: Option<&Scope>,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        let mut context = Context::new(self, input, portable)?;
        let plan = prepare::retrieval(&context.catalog, &context.attachments(), input)?;
        if matches!(plan.request.cache, CachePolicy::Cached { .. }) {
            context.durable(input, None)?;
        } else {
            context.store.metadata_only = true;
        }
        context.authorize(&plan.policies, input, scope, Action::Acquire)?;
        let result = if let Some(identity) = &self.session {
            AppSession {
                store: context.store.clone(),
                app: identity.app,
            }
            .retrieve(&plan.request, scope, options)?
        } else {
            context
                .store
                .retrieve_authorized(&plan.request, options, None)?
        };
        context.store.check_policy_fence()?;
        Ok(result)
    }
    pub fn compose_referenced(
        &self,
        input: &Retrieval,
        acquisition: &Acquisition,
        portable: &PolicyCatalog,
        options: &RetrieveOptions,
    ) -> Result<AcquiredResource> {
        ensure!(
            self.session.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "use scoped application acquisition"
            )
        );
        self.compose_referenced_scoped(input, acquisition, portable, None, options)
    }
    pub(crate) fn compose_referenced_scoped(
        &self,
        input: &Retrieval,
        acquisition: &Acquisition,
        portable: &PolicyCatalog,
        scope: Option<&Scope>,
        options: &RetrieveOptions,
    ) -> Result<AcquiredResource> {
        let mut context = Context::new(self, input, portable)?;
        let retrieval = prepare::retrieval(&context.catalog, &context.attachments(), input)?;
        let copied = prepare::acquisition(&context.catalog, &context.attachments(), acquisition)?;
        if matches!(acquisition.destination, Destination::Managed {}) {
            context.durable(input, Some(acquisition))?;
        } else if matches!(retrieval.request.cache, CachePolicy::Cached { .. }) {
            context.durable(input, None)?;
        } else {
            context.store.metadata_only = true;
        }
        context.authorize(&retrieval.policies, input, scope, Action::Acquire)?;
        context.authorize(&copied.policies, input, scope, Action::Acquire)?;
        context.store.policy_fence = context
            .store
            .policy_fence
            .take()
            .map(|fence| fence.with_authority(copied.policies, scope));
        if let Some(identity) = &self.session {
            AppSession {
                store: context.store,
                app: identity.app,
            }
            .acquire_composed(&retrieval.request, &copied.request, scope, options)
        } else {
            context.store.acquire_composed_authorized(
                &retrieval.request,
                &copied.request,
                options,
                None,
            )
        }
    }
}
impl AppSession {
    pub fn retrieve_referenced(
        &self,
        input: &Retrieval,
        portable: &PolicyCatalog,
        scope: Option<&Scope>,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        self.store
            .retrieve_referenced_scoped(input, portable, scope, options)
    }
    pub fn compose_referenced(
        &self,
        input: &Retrieval,
        acquisition: &Acquisition,
        portable: &PolicyCatalog,
        scope: Option<&Scope>,
        options: &RetrieveOptions,
    ) -> Result<AcquiredResource> {
        self.store
            .compose_referenced_scoped(input, acquisition, portable, scope, options)
    }
}
