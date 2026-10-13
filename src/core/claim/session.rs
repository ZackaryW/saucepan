//! App-facing handles carry identity through cached and direct UUID operations.
//! The Store itself is trusted local administration, not an app request surface.
use super::registry::CredentialChange;
use crate::{
    core::{Store, index::Journal, source::AcquireOptions, store::ResourceLease},
    error::{Error, ErrorCode},
    models::{
        claim::{
            Claim, ClaimContext, ClaimView, Claimed, RegistryState, Scope,
            policy::{AccessPolicy, Action, Registration},
            security::Credential,
        },
        index::{Application, Record},
        store::{Request, VerificationMode},
    },
};
use anyhow::{Result, ensure};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct SessionIdentity {
    pub app: Uuid,
    pub token: Option<Arc<Credential>>,
}
#[derive(Clone)]
pub struct AppSession {
    pub(crate) store: Store,
    pub(crate) app: Uuid,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationRegistration {
    pub registration_key: Scope,
    pub name: String,
    #[serde(default)]
    pub access: AccessPolicy,
    #[serde(default)]
    pub policies: Registration,
    #[serde(default)]
    pub credentials_required: bool,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationConfiguration {
    pub expected_revision: u64,
    pub access: AccessPolicy,
    pub policies: Registration,
    pub credential: Option<CredentialChange>,
}
pub struct RegisteredApplication {
    pub application: Application,
    /// Delivered once. Never serialized into application metadata or receipts.
    pub credential: Option<Credential>,
}

impl Store {
    pub fn session(&self, app: Uuid, credential: Option<Credential>) -> Result<AppSession> {
        ensure!(
            self.session.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "cannot switch application identity"
            )
        );
        let _lock = self.lock()?;
        self.application(app)?;
        let registry = self.registry(app)?;
        self.metadata.authenticator().check_credential(
            app,
            &registry.registry.credential,
            credential.as_ref(),
        )?;
        let mut store = self.clone();
        store.session = Some(SessionIdentity {
            app,
            token: credential.map(Arc::new),
        });
        Ok(AppSession { store, app })
    }

    pub fn register_application(
        &self,
        input: &ApplicationRegistration,
        proof: Option<&Credential>,
    ) -> Result<RegisteredApplication> {
        self.register_application_delivering(input, proof, |_| Ok(()))
    }

    /// Deliver a newly issued secret before committing the registration. A
    /// failed delivery leaves metadata unchanged; retries never reissue a token.
    pub fn register_application_delivering(
        &self,
        input: &ApplicationRegistration,
        proof: Option<&Credential>,
        deliver: impl FnOnce(&Credential) -> Result<()>,
    ) -> Result<RegisteredApplication> {
        ensure!(
            self.session.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "registration requires administration"
            )
        );
        ensure!(
            !input.name.trim().is_empty(),
            Error::new(ErrorCode::InvalidRequest, "empty application name")
        );
        input.policies.validate()?;
        let _lock = self.lock()?;
        for record in self.records()? {
            let Record::Application(app) = record else {
                continue;
            };
            let state = self.registry(app.uuid)?;
            if state.registry.registration_key != input.registration_key {
                continue;
            }
            self.metadata.authenticator().check_credential(
                app.uuid,
                &state.registry.credential,
                proof,
            )?;
            ensure!(
                app.name == input.name
                    && state.registry.access == input.access
                    && state.registry.policies == input.policies
                    && state.credentials_required() == input.credentials_required,
                Error::new(
                    ErrorCode::RevisionConflict,
                    "registration settings changed; use revision-checked configuration"
                )
            );
            ensure!(
                app.state == crate::models::index::AppState::Ready,
                Error::new(ErrorCode::AccessDenied, "application is removing")
            );
            return Ok(RegisteredApplication {
                application: app,
                credential: None,
            });
        }
        let app = Application {
            uuid: Uuid::new_v4(),
            name: input.name.clone(),
            settings: Default::default(),
            active: None,
            state: Default::default(),
            revision: 0,
            shims: Vec::new(),
        };
        let mut registry = Self::new_registry(app.uuid, app.revision)?;
        registry.registry.registration_key = input.registration_key.clone();
        registry.registry.access = input.access.clone();
        registry.registry.policies = input.policies.clone();
        let credential = if input.credentials_required {
            registry.change_credentials(
                self.metadata.authenticator(),
                None,
                CredentialChange::Enable,
            )?
        } else {
            None
        };
        if let Some(token) = &credential {
            deliver(token)?;
        }
        self.commit(Journal {
            records: vec![Record::Application(app.clone())],
            registries: vec![registry],
            ..Default::default()
        })?;
        Ok(RegisteredApplication {
            application: app,
            credential,
        })
    }

    /// Explicit trusted administrative mutation; current proof is still required
    /// when enabled. An app session never exposes or delegates this operation.
    pub fn configure_application(
        &self,
        app: Uuid,
        input: ApplicationConfiguration,
        proof: Option<&Credential>,
    ) -> Result<Option<Credential>> {
        self.configure_application_delivering(app, input, proof, |_| Ok(()))
    }

    pub fn configure_application_delivering(
        &self,
        app: Uuid,
        input: ApplicationConfiguration,
        proof: Option<&Credential>,
        deliver: impl FnOnce(&Credential) -> Result<()>,
    ) -> Result<Option<Credential>> {
        let ApplicationConfiguration {
            expected_revision,
            access,
            policies,
            credential,
        } = input;
        ensure!(
            self.session.is_none(),
            Error::new(
                ErrorCode::AccessDenied,
                "configuration requires administration"
            )
        );
        let _lock = self.lock()?;
        let mut application = self.application(app)?;
        let mut registry = self.registry(app)?;
        self.metadata.authenticator().check_credential(
            app,
            &registry.registry.credential,
            proof,
        )?;
        ensure!(
            application.revision == expected_revision,
            Error::new(ErrorCode::RevisionConflict, "application revision conflict")
        );
        registry.registry.access = access;
        registry.registry.policies = policies;
        registry.validate()?;
        let token = credential
            .map(|change| registry.change_credentials(self.metadata.authenticator(), proof, change))
            .transpose()?
            .flatten();
        if let Some(token) = &token {
            deliver(token)?;
        }
        application.bump()?;
        self.commit(Journal {
            records: vec![Record::Application(application)],
            registries: vec![registry],
            ..Default::default()
        })?;
        Ok(token)
    }

    pub(crate) fn authorize_resource(
        &self,
        registry: &RegistryState,
        action: Action,
        source: Option<Uuid>,
        scope: Option<&Scope>,
    ) -> Result<()> {
        if let Some(session) = &self.session {
            ensure!(
                session.app == registry.registry.app,
                Error::new(ErrorCode::AccessDenied, "application identity mismatch")
            );
            registry.authorize(
                self.metadata.authenticator(),
                session.token.as_deref(),
                action,
                source,
                scope,
            )?;
        } else {
            // Supplying an app UUID is still an app-scoped operation, even when
            // credentials are disabled. Maintenance does not use this grant path.
            registry.authorize(self.metadata.authenticator(), None, action, source, scope)?;
        }
        self.authorize_policy_refs(
            &registry.registry.policy_refs,
            source,
            scope,
            action,
            crate::core::policy::authority::Projection::Resource,
        )?;
        Ok(())
    }
    pub(crate) fn authorize_claim(
        &self,
        registry: &RegistryState,
        claim: &Claim,
        action: Action,
    ) -> Result<()> {
        self.authorize_resource(
            registry,
            action,
            Some(claim.request.source_uuid),
            Some(&claim.scope),
        )
    }
    pub(crate) fn check_session(&self, registry: &RegistryState) -> Result<()> {
        if let Some(session) = &self.session {
            ensure!(
                session.app == registry.registry.app,
                Error::new(ErrorCode::AccessDenied, "application identity mismatch")
            );
            self.metadata.authenticator().check_credential(
                session.app,
                &registry.registry.credential,
                session.token.as_deref(),
            )?;
        } else {
            self.metadata.authenticator().check_credential(
                registry.registry.app,
                &registry.registry.credential,
                None,
            )?;
        }
        Ok(())
    }

    pub(crate) fn authorize_output(&self, output: Uuid, action: Action) -> Result<()> {
        if let Some(session) = &self.session {
            self.application(session.app)?;
            let registry = self.registry(session.app)?;
            self.check_session(&registry)?;
            ensure!(
                registry
                    .claims
                    .values()
                    .any(|claim| claim.entry_uuid == output
                        && self.authorize_claim(&registry, claim, action).is_ok()),
                Error::new(
                    ErrorCode::AccessDenied,
                    "output is not accessible through an authorized claim"
                )
            );
        }
        Ok(())
    }
    pub(crate) fn authorize_binding(&self, app: Uuid, output: Uuid, action: Action) -> Result<()> {
        let registry = self.registry(app)?;
        self.check_session(&registry)?;
        if self.session.is_some() {
            ensure!(
                registry
                    .claims
                    .values()
                    .any(|claim| claim.entry_uuid == output
                        && self.authorize_claim(&registry, claim, action).is_ok()),
                Error::new(
                    ErrorCode::AccessDenied,
                    "binding is not accessible through an authorized claim"
                )
            );
        }
        Ok(())
    }
    pub(crate) fn authorize_application_action(&self, app: Uuid, action: Action) -> Result<()> {
        let registry = self.registry(app)?;
        self.check_session(&registry)?;
        for claim in registry.claims.values() {
            self.authorize_claim(&registry, claim, action)?;
        }
        if registry.claims.is_empty() {
            // No source/scope resources are affected, but the action itself still
            // needs permission. In particular an empty action set grants nothing.
            let mut empty = registry;
            empty.registry.access.sources = None;
            empty.registry.access.scopes = None;
            self.authorize_policy_refs(
                &empty.registry.policy_refs,
                None,
                None,
                action,
                crate::core::policy::authority::Projection::ActionOnly,
            )?;
            empty.registry.policy_refs = Default::default();
            self.authorize_resource(&empty, action, None, None)?;
        }
        Ok(())
    }
}

impl AppSession {
    pub(crate) fn into_store(self) -> Store {
        self.store
    }
    pub fn app(&self) -> Uuid {
        self.app
    }
    pub fn ensure(
        &self,
        input: &crate::models::claim::EnsureRequest,
        options: &AcquireOptions,
    ) -> Result<Claimed> {
        self.store.ensure_claim(self.app, input, options)
    }
    pub fn lease_view(
        &self,
        view: &crate::models::claim::CoveredView,
        mode: VerificationMode,
    ) -> Result<ResourceLease> {
        ensure!(
            view.app == self.app,
            Error::new(
                ErrorCode::AccessDenied,
                "view belongs to another application"
            )
        );
        self.store.lease_view(view, mode)
    }
    pub fn acquire(
        &self,
        scope: &Scope,
        request: &Request,
        context: &ClaimContext,
        options: &AcquireOptions,
    ) -> Result<Claimed> {
        self.store
            .acquire_for_with_context(self.app, scope, request, context, options)
    }
    pub fn update(&self, claim: Uuid, options: &AcquireOptions) -> Result<Claimed> {
        self.store.update_claim(self.app, claim, options)
    }
    pub fn recover(&self, claim: Uuid, options: &AcquireOptions) -> Result<Claimed> {
        self.store.recover_claim(self.app, claim, options)
    }
    pub fn release(&self, claim: Uuid) -> Result<()> {
        self.store.release_claim(self.app, claim)
    }
    pub fn set_context(&self, claim: Uuid, context: &ClaimContext, revision: u64) -> Result<Claim> {
        self.store
            .set_claim_context(self.app, claim, context, revision)
    }
    pub fn claims(&self) -> Result<Vec<ClaimView>> {
        self.store.claims(self.app)
    }
    pub fn query(
        &self,
        query: &crate::core::query::ClaimQuery,
    ) -> Result<crate::core::query::QueryResult> {
        ensure!(
            query.app == self.app,
            Error::new(
                ErrorCode::AccessDenied,
                "query belongs to another application"
            )
        );
        self.store.query_claims(query)
    }
    /// Find permitted registered sources before acquiring or creating claims.
    pub fn query_sources(
        &self,
        query: &crate::core::query::SourceQuery,
    ) -> Result<crate::core::query::SourceQueryResult> {
        self.store.query_sources(query)
    }
    pub fn lease_claim(&self, claim: Uuid, mode: VerificationMode) -> Result<ResourceLease> {
        self.store.lease_claim(self.app, claim, mode)
    }
    pub fn lease_output(&self, entry: Uuid, mode: VerificationMode) -> Result<ResourceLease> {
        self.store.lease(entry, mode)
    }
    pub fn activate(
        &self,
        entry: Uuid,
        executable: &str,
        probe: &crate::core::activation::Probe,
    ) -> Result<crate::models::index::Binding> {
        self.store.activate(self.app, entry, executable, probe)
    }
    pub fn deactivate(&self) -> Result<Option<crate::models::index::Binding>> {
        self.store.deactivate(self.app)
    }
    pub fn update_application(
        &self,
        claim: Uuid,
        probe: &crate::core::activation::Probe,
        rollback: Option<Uuid>,
        options: &AcquireOptions,
        operation: Option<Uuid>,
    ) -> Result<crate::models::index::lifecycle::Operation> {
        self.store
            .update_application(self.app, claim, probe, rollback, options, operation)
    }
    pub fn uninstall(
        &self,
        leave_unmanaged_shims: bool,
        operation: Option<Uuid>,
    ) -> Result<crate::models::index::lifecycle::Operation> {
        self.store
            .uninstall(self.app, leave_unmanaged_shims, operation)
    }
    pub fn operations(&self) -> Result<Vec<crate::models::index::lifecycle::Operation>> {
        let _lock = self.store.lock()?;
        self.store
            .authorize_application_action(self.app, Action::Read)?;
        let registry = self.store.registry(self.app)?;
        Ok(self
            .store
            .operations(Some(self.app))?
            .into_iter()
            .filter(|op| {
                op.desired_registry.as_ref().is_none_or(|desired| {
                    desired.claims.values().all(|claim| {
                        self.store
                            .authorize_claim(&registry, claim, Action::Read)
                            .is_ok()
                    })
                })
            })
            .collect())
    }
}
