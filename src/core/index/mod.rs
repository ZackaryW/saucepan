use crate::{
    core::{
        Store,
        claim::{
            persistence::ContentPublication,
            registry::RegistryState,
            security::{Identity, Kind},
        },
    },
    error::{Error, ErrorCode},
    models::{
        index::{Application, Record, Settings},
        source::{Source, SourceRecord},
    },
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{collections::BTreeMap, fs, path::Path};
use uuid::Uuid;

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Journal {
    pub records: Vec<Record>,
    pub registries: Vec<RegistryState>,
    pub pins: Vec<crate::models::pin::PinRecord>,
    pub stage: Option<(String, Uuid)>,
    #[serde(default)]
    pub retrieval_stage: Option<(String, Uuid)>,
    pub operation: Option<crate::models::index::lifecycle::Operation>,
    pub retire: Option<(Uuid, Uuid)>,
}

pub(crate) fn classify_missing(
    error: anyhow::Error,
    code: ErrorCode,
    message: &'static str,
) -> anyhow::Error {
    if error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
    {
        error.context(Error::new(code, message))
    } else {
        error
    }
}
/// Untrusted external request/sidecar reader. Authoritative metadata uses Metadata.
pub(crate) fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(&fs::read(path).with_context(|| format!("reading {}", path.display()))?)
        .map_err(|e| {
            anyhow::Error::new(e).context(Error::new(
                ErrorCode::IntegrityError,
                "invalid stored metadata",
            ))
        })
}
pub(crate) fn record_identity(record: &Record) -> Identity {
    match record {
        Record::Application(app) => Identity {
            kind: Kind::Application,
            owner: Some(app.uuid),
            member: None,
        },
        Record::Output(output) => Identity {
            kind: Kind::Output,
            owner: None,
            member: Some(output.uuid),
        },
        Record::Acquisition(record) => Identity {
            kind: Kind::Acquisition,
            owner: None,
            member: Some(record.uuid),
        },
        Record::Retrieval(record) => Identity {
            kind: Kind::Retrieval,
            owner: None,
            member: Some(record.uuid),
        },
    }
}
pub(crate) fn source_identity(id: Uuid) -> Identity {
    Identity {
        kind: Kind::Source,
        owner: None,
        member: Some(id),
    }
}
pub(crate) fn operation_identity(op: &crate::models::index::lifecycle::Operation) -> Identity {
    Identity {
        kind: Kind::Operation,
        owner: Some(op.app),
        member: Some(op.operation),
    }
}
impl Store {
    pub fn record(&self, id: Uuid) -> Result<Record> {
        let _lock = self.lock()?;
        let snapshot = self.metadata.snapshot()?;
        let reference = snapshot
            .catalog
            .records
            .get(&format!("index/{id}/index.json"))
            .with_context(|| Error::new(ErrorCode::UnknownOutput, "index record missing"))?;
        let record: Record = snapshot.read(self.metadata.authenticator(), reference.identity)?;
        ensure!(
            record.uuid() == id && record_identity(&record) == reference.identity,
            Error::new(ErrorCode::IntegrityError, "index identity mismatch")
        );
        Ok(record)
    }
    pub fn application(&self, id: Uuid) -> Result<Application> {
        let _lock = self.lock()?;
        let snapshot = self.metadata.snapshot()?;
        let record: Record = snapshot.read(
            self.metadata.authenticator(),
            Identity {
                kind: Kind::Application,
                owner: Some(id),
                member: None,
            },
        )?;
        match record {
            Record::Application(a) => {
                ensure!(
                    a.uuid == id,
                    Error::new(ErrorCode::IntegrityError, "application identity mismatch")
                );
                ensure!(
                    a.state == crate::models::index::AppState::Ready,
                    Error::new(ErrorCode::AccessDenied, "application is removing")
                );
                let registry = RegistryState::load(&self.metadata, &snapshot, id)?;
                ensure!(
                    registry.registry.revision == a.revision,
                    Error::new(
                        ErrorCode::IntegrityError,
                        "application registry revision mismatch"
                    )
                );
                Ok(a)
            }
            _ => bail!(Error::new(
                ErrorCode::UnknownApplication,
                "not an application"
            )),
        }
    }
    pub fn registry(&self, app: Uuid) -> Result<RegistryState> {
        let _lock = self.lock()?;
        let snapshot = self.metadata.snapshot()?;
        let record: Record = snapshot.read(
            self.metadata.authenticator(),
            Identity {
                kind: Kind::Application,
                owner: Some(app),
                member: None,
            },
        )?;
        let Record::Application(application) = record else {
            bail!(Error::new(
                ErrorCode::UnknownApplication,
                "not an application"
            ));
        };
        let registry = RegistryState::load(&self.metadata, &snapshot, app)?;
        ensure!(
            application.uuid == app && application.revision == registry.registry.revision,
            Error::new(
                ErrorCode::IntegrityError,
                "application registry revision mismatch"
            )
        );
        Ok(registry)
    }
    #[cfg(test)]
    pub(crate) fn save_record(&self, record: &Record) -> Result<()> {
        self.commit(Journal {
            records: vec![record.clone()],
            ..Default::default()
        })
    }
    pub fn records(&self) -> Result<Vec<Record>> {
        let _lock = self.lock()?;
        let snapshot = self.metadata.snapshot()?;
        let mut out = Vec::new();
        for reference in snapshot.catalog.records.values() {
            if matches!(
                reference.identity.kind,
                Kind::Application | Kind::Output | Kind::Retrieval | Kind::Acquisition
            ) {
                let record: Record =
                    snapshot.read(self.metadata.authenticator(), reference.identity)?;
                ensure!(
                    record_identity(&record) == reference.identity,
                    Error::new(ErrorCode::IntegrityError, "index identity mismatch")
                );
                if let Record::Application(app) = &record {
                    let registry = RegistryState::load(&self.metadata, &snapshot, app.uuid)?;
                    ensure!(
                        app.revision == registry.registry.revision,
                        Error::new(ErrorCode::IntegrityError, "registry revision mismatch")
                    );
                }
                out.push(record);
            }
        }
        Ok(out)
    }
    pub fn register_app(&self, name: String) -> Result<Application> {
        ensure!(
            !name.trim().is_empty(),
            Error::new(ErrorCode::InvalidRequest, "empty app name")
        );
        let _guard = self.lock()?;
        let app = Application {
            uuid: Uuid::new_v4(),
            name,
            settings: Settings::default(),
            active: None,
            state: Default::default(),
            revision: 0,
            shims: Vec::new(),
        };
        let registry = Self::new_registry(app.uuid, app.revision)?;
        self.commit(Journal {
            records: vec![Record::Application(app.clone())],
            registries: vec![registry],
            ..Default::default()
        })?;
        Ok(app)
    }
    pub(crate) fn new_registry(app: Uuid, revision: u64) -> Result<RegistryState> {
        Ok(RegistryState {
            registry: crate::models::claim::Registry {
                policy_refs: Default::default(),
                app,
                revision,
                registration_key: app.to_string().try_into()?,
                access: Default::default(),
                credential: Default::default(),
                members: BTreeMap::new(),
            },
            claims: BTreeMap::new(),
        })
    }
    pub fn register_source(&self, source: Source) -> Result<SourceRecord> {
        let source = super::source::git::normalize(source).map_err(|e| {
            e.context(Error::new(
                ErrorCode::InvalidRequest,
                "invalid source definition",
            ))
        })?;
        let _guard = self.lock()?;
        if let Some(existing) = self.find_source(&source)? {
            return Ok(existing);
        }
        let record = SourceRecord {
            policy_refs: Default::default(),
            policy_revision: 0,
            uuid: Uuid::new_v4(),
            source,
            public_discovery: false,
            refresh: Default::default(),
            retrieval_freshness: None,
            discovery: BTreeMap::new(),
        };
        self.save_source(&record)?;
        Ok(record)
    }

    /// Look up an already normalized source without registration effects.
    pub(crate) fn find_source(&self, source: &Source) -> Result<Option<SourceRecord>> {
        let _guard = self.lock()?;
        let snapshot = self.metadata.snapshot()?;
        let policies = crate::core::policy::persistence::load(&self.metadata, &snapshot)?;
        for reference in snapshot
            .catalog
            .records
            .values()
            .filter(|r| r.identity.kind == Kind::Source)
        {
            let mut existing: SourceRecord =
                snapshot.read(self.metadata.authenticator(), reference.identity)?;
            crate::core::policy::source::hydrate(&mut existing, &policies)?;
            if &existing.source == source {
                return Ok(Some(existing));
            }
        }
        Ok(None)
    }
    pub fn source(&self, id: Uuid) -> Result<SourceRecord> {
        let _lock = self.lock()?;
        let snapshot = self.metadata.snapshot()?;
        let mut record: SourceRecord =
            snapshot.read(self.metadata.authenticator(), source_identity(id))?;
        crate::core::policy::source::hydrate(
            &mut record,
            &crate::core::policy::persistence::load(&self.metadata, &snapshot)?,
        )?;
        ensure!(
            record.uuid == id,
            Error::new(ErrorCode::IntegrityError, "source identity mismatch")
        );
        Ok(record)
    }
    pub(crate) fn save_source(&self, source: &SourceRecord) -> Result<()> {
        self.check_policy_fence()?;
        ensure!(
            !self.metadata_only,
            Error::new(
                ErrorCode::AccessDenied,
                "metadata-only access cannot publish sources"
            )
        );
        let snapshot = self.metadata.snapshot()?;
        self.metadata.publish(
            &snapshot,
            vec![(source_identity(source.uuid), serde_json::to_value(source)?)],
            vec![],
        )?;
        Ok(())
    }
    /// Trusted local administration. Publishing exposes the full source definition.
    pub fn set_source_visibility(&self, id: Uuid, public: bool) -> Result<SourceRecord> {
        self.source_behavior(
            id,
            crate::models::policy::record::PolicyDefinition::SourceVisibilityPolicy {
                public_discovery: public,
            },
        )
    }
    pub(crate) fn commit(&self, journal: Journal) -> Result<()> {
        self.commit_observed(journal, |_| Ok(()))
    }
    pub(crate) fn commit_observed(
        &self,
        mut journal: Journal,
        observer: impl FnMut(crate::core::claim::persistence::Publication) -> crate::error::Result<()>,
    ) -> Result<()> {
        self.check_policy_fence()?;
        ensure!(
            !self.metadata_only,
            Error::new(
                ErrorCode::AccessDenied,
                "metadata-only access cannot commit changes"
            )
        );
        let snapshot = self.metadata.snapshot()?;
        let mut writes = Vec::new();
        let mut deletes = Vec::new();
        for pin in &journal.pins {
            writes.push((crate::core::pin::identity(pin), serde_json::to_value(pin)?));
        }
        for record in &journal.records {
            writes.push((record_identity(record), serde_json::to_value(record)?));
            if let Record::Application(app) = record {
                if let Some(registry) = journal
                    .registries
                    .iter_mut()
                    .find(|r| r.registry.app == app.uuid)
                {
                    registry.registry.revision = app.revision;
                } else {
                    let mut registry = RegistryState::load(&self.metadata, &snapshot, app.uuid)?;
                    registry.registry.revision = app.revision;
                    journal.registries.push(registry);
                }
            }
        }
        for registry in &journal.registries {
            let encoded = registry.encode(&self.metadata, &snapshot)?;
            writes.extend(encoded.writes);
            deletes.extend(encoded.deletes);
        }
        if let Some(operation) = &journal.operation {
            writes.push((
                operation_identity(operation),
                serde_json::to_value(operation)?,
            ));
        }
        if let Some((app, _operation)) = journal.retire {
            let registry = RegistryState::load(&self.metadata, &snapshot, app)?;
            ensure!(registry.claims.is_empty(), "application still owns claims");
            let identity = Identity {
                kind: Kind::Application,
                owner: Some(app),
                member: None,
            };
            let record: Record = snapshot.read(self.metadata.authenticator(), identity)?;
            ensure!(
                matches!(record, Record::Application(ref application) if application.uuid == app && application.state == crate::models::index::AppState::Removing && application.active.is_none()),
                "application is not ready for retirement"
            );
            deletes.extend(RegistryState::retire(&snapshot, app)?);
            deletes.push(identity);
        }
        let content = journal
            .stage
            .map(|(stage, output)| {
                let files = journal
                    .records
                    .iter()
                    .find_map(|r| match r {
                        Record::Output(e) if e.uuid == output => Some(e.files.clone()),
                        _ => None,
                    })
                    .context("publication lacks output record")?;
                Ok::<_, anyhow::Error>(ContentPublication {
                    stage,
                    output,
                    retrieval_source: None,
                    files,
                })
            })
            .transpose()?;
        ensure!(
            content.is_none() || journal.retrieval_stage.is_none(),
            "one content publication per transaction"
        );
        let retrieval = journal
            .retrieval_stage
            .map(|(stage, id)| {
                let record = journal
                    .records
                    .iter()
                    .find_map(|record| match record {
                        Record::Retrieval(record) if record.uuid == id => Some(record),
                        _ => None,
                    })
                    .context("publication lacks retrieval record")?;
                Ok::<_, anyhow::Error>(ContentPublication {
                    stage,
                    output: id,
                    retrieval_source: Some(record.source_uuid),
                    files: record.files.clone(),
                })
            })
            .transpose()?;
        self.metadata.publish_staged(
            &snapshot,
            writes,
            deletes,
            content.or(retrieval),
            observer,
        )?;
        if let Some(fence) = &self.policy_fence {
            for record in &journal.records {
                if let Record::Application(app) = record {
                    fence.published(app.uuid, app.revision);
                }
            }
        }
        Ok(())
    }
    pub(crate) fn recover_locked(&self) -> Result<()> {
        self.metadata.recover()?;
        self.metadata.snapshot()?;
        Ok(())
    }
}
#[cfg(test)]
mod tests;
