//! Read-only tracing of the execution evaluator under the store mutation boundary.
use super::{
    catalog::PolicyCatalog,
    claims, composition, persistence,
    prepare::{self, Attachments},
    resolve::{self, Operation},
};
use crate::{
    core::Store,
    error::{Error, ErrorCode},
    models::{
        claim::policy::Action,
        policy::{
            PolicyDefinition, PolicyRefs,
            effective::EvaluatedPolicies,
            explanation::{
                AxisExplanation, ExplainRequest, Origin, PolicyExplanation, RestrictionEvidence,
            },
            record::Axis,
        },
        retrieval::SourceInput,
        store::Destination,
    },
};
use anyhow::{Result, ensure};
use std::collections::BTreeMap;

const AXES: [Axis; 13] = [
    Axis::SourceVisibility,
    Axis::ClaimVisibility,
    Axis::Cache,
    Axis::Refresh,
    Axis::Stale,
    Axis::PayloadVerification,
    Axis::PathVerification,
    Axis::Selection,
    Axis::Identical,
    Axis::Update,
    Axis::Revision,
    Axis::Reclaim,
    Axis::Protection,
];

impl crate::core::claim::session::AppSession {
    pub fn explain_policies(
        &self,
        input: &ExplainRequest,
        portable: &PolicyCatalog,
    ) -> Result<PolicyExplanation> {
        self.store.explain_policies(input, portable)
    }
}

fn describe(
    catalog: &PolicyCatalog,
    plan: &EvaluatedPolicies,
    operation: Operation,
    attachments: &Attachments<'_>,
    request: &PolicyRefs,
    adopted: bool,
    managed: bool,
) -> Result<PolicyExplanation> {
    let behavior = serde_json::to_value(&plan.behavior)?;
    let mut axes = BTreeMap::new();
    for axis in AXES {
        let applicable = operation.accepts(axis)
            && (managed || !matches!(axis, Axis::Revision | Axis::Protection));
        let key = match axis {
            Axis::Refresh => "freshness",
            Axis::Stale => "allow_stale",
            Axis::Protection => "useable_app",
            Axis::Cache => "cache",
            Axis::PayloadVerification => "payload_verification",
            Axis::PathVerification => "path_verification",
            Axis::Selection => "selection",
            Axis::Identical => "identical",
            Axis::Update => "update",
            Axis::Revision => "revision",
            Axis::Reclaim => "reclaim",
            Axis::SourceVisibility => "public_discovery",
            Axis::ClaimVisibility => "public_scopes",
        };
        let evidence = applicable
            .then(|| plan.evidence.get(&axis).cloned())
            .flatten();
        let origin = applicable.then(|| {
            if adopted {
                Origin::AdoptedClaim
            } else if let Some(e) = &evidence {
                if request.ids().contains(&e.uuid) {
                    Origin::Request
                } else if attachments.application.ids().contains(&e.uuid) {
                    Origin::Application
                } else {
                    Origin::Source
                }
            } else {
                Origin::BuiltIn
            }
        });
        axes.insert(
            axis,
            AxisExplanation {
                applicable,
                value: applicable.then(|| behavior[key].clone()),
                origin,
                evidence,
            },
        );
    }
    let mut restrictions = Vec::new();
    for (refs, application) in [(attachments.source, false), (attachments.application, true)] {
        for id in refs.ids() {
            let record = catalog.get(*id)?;
            if matches!(
                record.definition,
                PolicyDefinition::AccessPolicy(_) | PolicyDefinition::PolicyConstraints(_)
            ) {
                restrictions.push(RestrictionEvidence {
                    origin: if application {
                        Origin::Application
                    } else {
                        Origin::Source
                    },
                    record: record.clone(),
                    digest: resolve::evidence(record)?.digest,
                });
            }
        }
    }
    Ok(PolicyExplanation { axes, restrictions })
}

fn planned(
    input: &ExplainRequest,
    catalog: &PolicyCatalog,
    attachments: &Attachments<'_>,
) -> Result<(PolicyExplanation, Vec<EvaluatedPolicies>)> {
    match input {
        ExplainRequest::Retrieve { request, .. } => {
            let p = prepare::retrieval(catalog, attachments, request)?;
            Ok((
                describe(
                    catalog,
                    &p.policies,
                    Operation::Retrieve,
                    attachments,
                    &request.policy_refs,
                    false,
                    false,
                )?,
                vec![p.policies],
            ))
        }
        ExplainRequest::Acquire { request, .. } => {
            let p = prepare::acquisition(catalog, attachments, request)?;
            Ok((
                describe(
                    catalog,
                    &p.policies,
                    Operation::Acquire,
                    attachments,
                    &request.policy_refs,
                    false,
                    matches!(request.destination, Destination::Managed {}),
                )?,
                vec![p.policies],
            ))
        }
        ExplainRequest::Compose {
            retrieval,
            acquisition,
            ..
        } => {
            let r = prepare::retrieval(catalog, attachments, retrieval)?;
            let a = prepare::acquisition(catalog, attachments, acquisition)?;
            composition::validate_composition(&r.request, &a.request, false)?;
            let mut report = describe(
                catalog,
                &r.policies,
                Operation::Retrieve,
                attachments,
                &retrieval.policy_refs,
                false,
                false,
            )?;
            let copied = describe(
                catalog,
                &a.policies,
                Operation::Acquire,
                attachments,
                &acquisition.policy_refs,
                false,
                matches!(acquisition.destination, Destination::Managed {}),
            )?;
            for (axis, value) in copied.axes {
                if value.applicable {
                    report.axes.insert(axis, value);
                }
            }
            Ok((report, vec![r.policies, a.policies]))
        }
        ExplainRequest::Claim { .. } => Err(Error::new(
            ErrorCode::InvalidRequest,
            "claim explanation requires a store",
        )
        .into()),
    }
}

pub fn portable(input: &ExplainRequest, catalog: &PolicyCatalog) -> Result<PolicyExplanation> {
    ensure!(
        matches!(input.source(), Some(SourceInput::Inline { .. })),
        Error::new(
            ErrorCode::InvalidRequest,
            "registered explanation requires a store"
        )
    );
    let empty = PolicyRefs::default();
    Ok(planned(
        input,
        catalog,
        &Attachments {
            source: &empty,
            application: &empty,
        },
    )?
    .0)
}

impl Store {
    pub fn explain_policies(
        &self,
        input: &ExplainRequest,
        portable: &PolicyCatalog,
    ) -> Result<PolicyExplanation> {
        // Observation must not replay a pending transaction, even through an
        // administrative Store handle. Reuse the existing read-only boundary.
        if !self.metadata_only {
            let mut reader = self.clone();
            reader.metadata_only = true;
            return reader.explain_policies(input, portable);
        }
        let _lock = self.lock()?;
        let snapshot = self.metadata.snapshot()?;
        let registered = persistence::load(&self.metadata, &snapshot)?;
        let catalog = registered.with_portable(portable)?;
        if let ExplainRequest::Claim { app, claim } = input {
            if let Some(session) = &self.session {
                ensure!(
                    session.app == *app,
                    Error::new(ErrorCode::AccessDenied, "application identity mismatch")
                );
            }
            self.application(*app)?;
            let registry = self.registry(*app)?;
            self.check_session(&registry)?;
            let claim = registry
                .claims
                .get(claim)
                .ok_or_else(|| Error::new(ErrorCode::InvalidRequest, "unknown claim"))?;
            self.authorize_claim(&registry, claim, Action::Read)?;
            let source = self.source(claim.request.source_uuid)?;
            let plan = claims::validate_existing(
                &catalog,
                claim,
                &source.policy_refs,
                &registry.registry.policy_refs,
            )?;
            return describe(
                &catalog,
                &plan,
                Operation::Compose,
                &Attachments {
                    source: &source.policy_refs,
                    application: &registry.registry.policy_refs,
                },
                &claim.policy_refs,
                true,
                true,
            );
        }
        let source_uuid = match input.source() {
            Some(SourceInput::Registered { source_uuid }) => Some(*source_uuid),
            _ => None,
        };
        let source_refs = source_uuid
            .map(|id| self.source(id).map(|s| s.policy_refs))
            .transpose()?
            .unwrap_or_default();
        let registry = self
            .session
            .as_ref()
            .map(|s| self.registry(s.app))
            .transpose()?;
        let empty = PolicyRefs::default();
        let app_refs = registry
            .as_ref()
            .map(|r| &r.registry.policy_refs)
            .unwrap_or(&empty);
        if let Some(registry) = &registry {
            self.check_session(registry)?;
            self.authorize_resource(registry, Action::Read, source_uuid, input.scope())?;
        }
        let (report, plans) = planned(
            input,
            &catalog,
            &Attachments {
                source: &source_refs,
                application: app_refs,
            },
        )?;
        for plan in plans {
            resolve::authorize(&plan, Action::Read, source_uuid, input.scope())?;
        }
        // Durable requests cannot use unregistered leaf identities, even in preview.
        match input {
            ExplainRequest::Retrieve { request, .. } => {
                let r = prepare::retrieval(
                    &catalog,
                    &Attachments {
                        source: &source_refs,
                        application: app_refs,
                    },
                    request,
                )?;
                if r.policies.behavior.cache == crate::models::policy::record::CacheMode::Cached {
                    prepare::require_registered(
                        &registered,
                        &catalog,
                        request
                            .policy_refs
                            .ids()
                            .iter()
                            .chain(request.asset.filter_refs.ids())
                            .copied(),
                    )?;
                }
            }
            ExplainRequest::Acquire { request, .. }
                if matches!(request.destination, Destination::Managed {}) =>
            {
                prepare::require_registered(
                    &registered,
                    &catalog,
                    request.policy_refs.ids().iter().copied(),
                )?;
            }
            ExplainRequest::Compose {
                retrieval,
                acquisition,
                ..
            } => {
                let r = prepare::retrieval(
                    &catalog,
                    &Attachments {
                        source: &source_refs,
                        application: app_refs,
                    },
                    retrieval,
                )?;
                if r.policies.behavior.cache == crate::models::policy::record::CacheMode::Cached {
                    prepare::require_registered(
                        &registered,
                        &catalog,
                        retrieval
                            .policy_refs
                            .ids()
                            .iter()
                            .chain(retrieval.asset.filter_refs.ids())
                            .copied(),
                    )?;
                }
                if matches!(acquisition.destination, Destination::Managed {}) {
                    prepare::require_registered(
                        &registered,
                        &catalog,
                        acquisition.policy_refs.ids().iter().copied(),
                    )?;
                }
            }
            _ => {}
        }
        Ok(report)
    }
}
