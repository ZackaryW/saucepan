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
    pub fn open_caller_referenced(
        input: &Retrieval,
        descriptor: &crate::models::retrieval::RetrievedDescriptor,
        catalog: &PolicyCatalog,
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
        Self::open_caller_planned(&plan.request, descriptor)
    }
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
        Self::retrieve_planned_with(&plan.request, options)
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
        let (request, store) = acquisition_for_handle(retrieved, input, portable, scope)?;
        // The borrowed original keeps its source guard alive throughout this
        // call. Only the derived execution context is replaced.
        let checked = RetrievedResource {
            request: retrieved.request.clone(),
            source: retrieved.source.clone(),
            store,
            authorization: retrieved.authorization.clone(),
            descriptor: retrieved.descriptor.clone(),
            _temporary: retrieved._temporary.clone(),
            _coordination: None,
        };
        Self::acquire_planned(&checked, &request)
    }
    /// Own the handle when an automatic update may replace its source lease.
    pub fn acquire_or_refresh_referenced(
        mut retrieved: RetrievedResource,
        input: &Acquisition,
        portable: &PolicyCatalog,
        scope: Option<&Scope>,
        options: &RetrieveOptions,
    ) -> Result<AcquiredResource> {
        let (request, store) = acquisition_for_handle(&retrieved, input, portable, scope)?;
        retrieved.store = store;
        if matches!(
            request.update_policy,
            crate::models::policy::UpdatePolicy::Manual {}
        ) {
            Self::acquire_planned(&retrieved, &request)
        } else {
            Self::refresh_planned_with(retrieved, &request, options)
        }
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
        Self::compose_planned(&retrieval.request, &acquisition.request, options)
    }
}

fn acquisition_for_handle(
    retrieved: &RetrievedResource,
    input: &Acquisition,
    portable: &PolicyCatalog,
    scope: Option<&Scope>,
) -> Result<(crate::models::store::AcquisitionPlan, Option<Store>)> {
    let empty = PolicyRefs::default();
    if let Some(store) = &retrieved.store {
        let source_input = Retrieval {
            source: retrieved.request.source.clone(),
            selector: retrieved.request.selector.clone(),
            asset: Default::default(),
            policy_refs: Default::default(),
            destination: None,
            expected_sha256: None,
        };
        let mut context = Context::new(store, &source_input, portable)?;
        let plan = prepare::acquisition(&context.catalog, &context.attachments(), input)?;
        if matches!(input.destination, Destination::Managed {}) {
            context.durable(&source_input, Some(input))?;
        }
        context.authorize(&plan.policies, &source_input, scope, Action::Acquire)?;
        context.store.policy_fence = context
            .store
            .policy_fence
            .take()
            .map(|fence| fence.with_authority(plan.policies, scope));
        Ok((plan.request, Some(context.store)))
    } else {
        Ok((
            prepare::acquisition(
                portable,
                &Attachments {
                    source: &empty,
                    application: &empty,
                },
                input,
            )?
            .request,
            None,
        ))
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
    /// Observational routing for front ends; the executing operation resolves and
    /// fences its own snapshot again before provider work.
    pub(crate) fn referenced_cache_mode(
        &self,
        input: &Retrieval,
        portable: &PolicyCatalog,
    ) -> Result<crate::models::policy::record::CacheMode> {
        let context = Context::new(self, input, portable)?;
        Ok(
            prepare::retrieval(&context.catalog, &context.attachments(), input)?
                .policies
                .behavior
                .cache,
        )
    }

    pub fn open_caller_referenced(
        &self,
        input: &Retrieval,
        descriptor: &crate::models::retrieval::RetrievedDescriptor,
        portable: &PolicyCatalog,
        scope: Option<&Scope>,
    ) -> Result<RetrievedResource> {
        let mut context = Context::new(self, input, portable)?;
        context.store.metadata_only = true;
        let plan = prepare::retrieval(&context.catalog, &context.attachments(), input)?;
        context.authorize(&plan.policies, input, scope, Action::Acquire)?;
        let authorization = match &self.session {
            Some(identity) => {
                let SourceInput::Registered { source_uuid } = input.source else {
                    return Err(Error::new(
                        ErrorCode::AccessDenied,
                        "application material requires a registered source",
                    )
                    .into());
                };
                Some(
                    context
                        .store
                        .begin_composition(identity.app, scope, source_uuid, None)?,
                )
            }
            None => None,
        };
        context
            .store
            .open_caller_authorized(&plan.request, descriptor, authorization)
    }

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
            .retrieve_planned(&plan.request, scope, options)?
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
            .compose_planned(&retrieval.request, &copied.request, scope, options)
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
    pub fn open_caller_referenced(
        &self,
        input: &Retrieval,
        descriptor: &crate::models::retrieval::RetrievedDescriptor,
        portable: &PolicyCatalog,
        scope: Option<&Scope>,
    ) -> Result<RetrievedResource> {
        self.store
            .open_caller_referenced(input, descriptor, portable, scope)
    }

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

impl Retriever {
    pub fn retrieve(input: &Retrieval, catalog: &PolicyCatalog) -> Result<RetrievedResource> {
        Self::retrieve_referenced(input, catalog, &Default::default())
    }
}
impl Store {
    pub fn retrieve(&self, input: &Retrieval) -> Result<RetrievedResource> {
        self.retrieve_with(input, &Default::default())
    }
    pub fn retrieve_with(
        &self,
        input: &Retrieval,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        self.retrieve_referenced(input, &PolicyCatalog::default(), options)
    }
}

impl Acquirer {
    pub fn acquire(retrieved: &RetrievedResource, input: &Acquisition) -> Result<AcquiredResource> {
        let scope = retrieved
            .authorization
            .as_ref()
            .and_then(|a| a.owner.scope.as_ref());
        Self::acquire_referenced(retrieved, input, &PolicyCatalog::default(), scope)
    }
    pub fn refresh_with(
        retrieved: RetrievedResource,
        input: &Acquisition,
        options: &RetrieveOptions,
    ) -> Result<AcquiredResource> {
        let scope = retrieved
            .authorization
            .as_ref()
            .and_then(|a| a.owner.scope.clone());
        Self::acquire_or_refresh_referenced(
            retrieved,
            input,
            &PolicyCatalog::default(),
            scope.as_ref(),
            options,
        )
    }
}
impl Store {
    pub fn acquire_composed(
        &self,
        retrieval: &Retrieval,
        acquisition: &Acquisition,
    ) -> Result<AcquiredResource> {
        self.acquire_composed_with(retrieval, acquisition, &Default::default())
    }
    pub fn acquire_composed_with(
        &self,
        retrieval: &Retrieval,
        acquisition: &Acquisition,
        options: &RetrieveOptions,
    ) -> Result<AcquiredResource> {
        self.compose_referenced(retrieval, acquisition, &PolicyCatalog::default(), options)
    }
}
impl AppSession {
    pub fn retrieve(
        &self,
        input: &Retrieval,
        scope: Option<&Scope>,
        options: &RetrieveOptions,
    ) -> Result<RetrievedResource> {
        self.retrieve_referenced(input, &PolicyCatalog::default(), scope, options)
    }
    pub fn acquire_composed(
        &self,
        input: &Retrieval,
        acquisition: &Acquisition,
        scope: Option<&Scope>,
        options: &RetrieveOptions,
    ) -> Result<AcquiredResource> {
        self.compose_referenced(
            input,
            acquisition,
            &PolicyCatalog::default(),
            scope,
            options,
        )
    }
}
