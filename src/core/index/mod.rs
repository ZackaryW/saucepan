use crate::error::{Error, ErrorCode};
use crate::{
    core::Store,
    models::{
        index::{Application, Record, Settings},
        source::{Source, SourceRecord},
    },
    utils::{fs::atomic_write, path::beneath},
};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use serde::{Serialize, de::DeserializeOwned};
use std::{collections::BTreeMap, fs, path::Path};
use uuid::Uuid;
pub(crate) mod claims;

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Journal {
    pub records: Vec<Record>,
    pub stage: Option<(String, Uuid)>,
    #[serde(default)]
    pub operation: Option<crate::models::index::lifecycle::Operation>,
    #[serde(default)]
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

pub(crate) fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(&fs::read(path).with_context(|| format!("reading {}", path.display()))?)
        .map_err(|e| {
            anyhow::Error::new(e).context(Error::new(
                ErrorCode::IntegrityError,
                "invalid stored metadata",
            ))
        })
}
pub(crate) fn write(path: &Path, value: &impl Serialize) -> Result<()> {
    atomic_write(path, |file| {
        serde_json::to_writer_pretty(file, value).map_err(std::io::Error::other)
    })?;
    Ok(())
}
impl Store {
    pub fn record(&self, id: Uuid) -> Result<Record> {
        let record: Record = read(&beneath(&self.root, &format!("index/{id}/index.json"))?)?;
        ensure!(
            record.uuid() == id,
            Error::new(ErrorCode::IntegrityError, "index identity mismatch")
        );
        Ok(record)
    }
    pub fn application(&self, id: Uuid) -> Result<Application> {
        match self.record(id).map_err(|e| {
            classify_missing(e, ErrorCode::UnknownApplication, "application missing")
        })? {
            Record::Application(a) => {
                ensure!(
                    a.state == crate::models::index::AppState::Ready,
                    Error::new(ErrorCode::AccessDenied, "application is removing")
                );
                Ok(a)
            }
            _ => bail!(Error::new(
                ErrorCode::UnknownApplication,
                "not an application"
            )),
        }
    }
    pub(crate) fn save_record(&self, record: &Record) -> Result<()> {
        let dir = beneath(&self.root, &format!("index/{}", record.uuid()))?;
        if dir.exists() {
            write(&dir.join("index.json"), record)
        } else {
            crate::utils::fs::staged_directory(&dir, |stage| {
                let file = fs::File::create(stage.join("index.json"))?;
                serde_json::to_writer_pretty(&file, record).map_err(std::io::Error::other)?;
                file.sync_all()
            })?;
            Ok(())
        }
    }
    pub fn records(&self) -> Result<Vec<Record>> {
        let mut out = Vec::new();
        for entry in fs::read_dir(self.root.join("index"))? {
            let entry = entry?;
            let id = Uuid::parse_str(&entry.file_name().to_string_lossy())
                .context("invalid index directory UUID")?;
            out.push(self.record(id)?);
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
            claims: BTreeMap::new(),
            active: None,
            state: Default::default(),
            revision: 0,
            shims: Vec::new(),
        };
        self.save_record(&Record::Application(app.clone()))?;
        Ok(app)
    }
    pub fn register_source(&self, source: Source) -> Result<SourceRecord> {
        let source = super::source::git::normalize(source).map_err(|e| {
            e.context(Error::new(
                ErrorCode::InvalidRequest,
                "invalid source definition",
            ))
        })?;
        let _guard = self.lock()?;
        for entry in fs::read_dir(self.root.join("source"))? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "json") {
                let existing: SourceRecord = read(&path)?;
                if existing.source == source {
                    return Ok(existing);
                }
            }
        }
        let record = SourceRecord {
            uuid: Uuid::new_v4(),
            source,
            refresh: Default::default(),
            discovery: BTreeMap::new(),
        };
        self.save_source(&record)?;
        Ok(record)
    }
    pub fn source(&self, id: Uuid) -> Result<SourceRecord> {
        let record: SourceRecord = read(&beneath(&self.root, &format!("source/{id}.json"))?)
            .map_err(|e| classify_missing(e, ErrorCode::SourceError, "source missing"))?;
        ensure!(
            record.uuid == id,
            Error::new(ErrorCode::IntegrityError, "source identity mismatch")
        );
        Ok(record)
    }
    pub(crate) fn save_source(&self, source: &SourceRecord) -> Result<()> {
        write(
            &beneath(&self.root, &format!("source/{}.json", source.uuid))?,
            source,
        )
    }
}

impl Store {
    pub(crate) fn commit(&self, journal: Journal) -> Result<()> {
        write(&self.root.join(".staging/transaction.json"), &journal)?;
        self.recover_locked()
    }
    pub(crate) fn recover_locked(&self) -> Result<()> {
        let journal_path = self.root.join(".staging/transaction.json");
        if !journal_path.exists() {
            return Ok(());
        }
        let journal: Journal = read(&journal_path)?;
        if let Some((stage, id)) = &journal.stage {
            ensure!(
                !stage.contains(['/', '\\']) && !stage.is_empty(),
                "invalid staged directory"
            );
            let input = beneath(&self.root, &format!(".staging/{stage}/content"))?;
            let output = beneath(&self.root, &format!("store/{id}"))?;
            let record = journal
                .records
                .iter()
                .find_map(|r| match r {
                    Record::Output(e) if e.uuid == *id => Some(e),
                    _ => None,
                })
                .context("publication lacks output record")?;
            if output.exists() {
                ensure!(
                    super::store::manifest(&output)? == record.files,
                    "incomplete published output"
                );
            } else if input.exists() {
                ensure!(
                    super::store::manifest(&input)? == record.files,
                    "staged integrity mismatch"
                );
                fs::rename(input, output)?;
            } else {
                // Nothing published: leave old claims intact and abandon this intent.
                fs::remove_file(journal_path)?;
                return Ok(());
            }
        }
        for record in &journal.records {
            self.save_record(record)?;
        }
        if let Some((app, operation)) = journal.retire
            && let Err(error) = self.retire_application_locked(app, operation)
        {
            let mut op = journal
                .operation
                .clone()
                .context("retirement requires operation receipt")?;
            op.status = crate::models::index::lifecycle::Phase::CleanupPending;
            op.pending = vec![format!("index/{app}: {error:#}")];
            self.save_operation(&op)?;
            fs::remove_file(journal_path)?;
            return Ok(());
        }
        if let Some(operation) = &journal.operation {
            self.save_operation(operation)?;
        }
        fs::remove_file(journal_path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::store::{Request, Selector};
    #[test]
    fn interrupted_deactivation_recovers_before_and_after_record_publication() {
        for published in [false, true] {
            let tmp = tempfile::tempdir().unwrap();
            let store = Store::create(&tmp.path().join("root")).unwrap();
            let mut app = store.register_app("deactivation recovery".into()).unwrap();
            let source = Uuid::new_v4();
            let claim = Uuid::new_v4();
            let target = Uuid::new_v4();
            app.settings.source_filter = vec![source];
            app.claims.insert(
                claim,
                crate::models::index::Claim {
                    context: Default::default(),
                    uuid: claim,
                    entry_uuid: target,
                    request: Request {
                        source_uuid: source,
                        selector: Selector::Local,
                        selection: Default::default(),
                        policies: Default::default(),
                    },
                },
            );
            app.active = Some(crate::models::index::Binding {
                entry_uuid: target,
                executable: "tool".into(),
            });
            store
                .save_record(&Record::Application(app.clone()))
                .unwrap();
            app.active = None;
            let inactive = Record::Application(app.clone());
            write(
                &store.root.join(".staging/transaction.json"),
                &Journal {
                    records: vec![inactive.clone()],
                    stage: None,
                    ..Default::default()
                },
            )
            .unwrap();
            if published {
                store.save_record(&inactive).unwrap();
            }
            let reopened = Store::open(&store.root).unwrap();
            assert_eq!(
                serde_json::to_value(reopened.application(app.uuid).unwrap()).unwrap(),
                serde_json::to_value(app).unwrap()
            );
            assert!(!store.root.join(".staging/transaction.json").exists());
        }
    }

    #[test]
    fn interrupted_publication_recovers_before_and_after_rename() {
        for (renamed, corrupt) in [(false, false), (true, false), (false, true), (true, true)] {
            let tmp = tempfile::tempdir().unwrap();
            let input = tmp.path().join("file");
            fs::write(&input, "complete").unwrap();
            let store = Store::create(&tmp.path().join("root")).unwrap();
            let source = store
                .register_source(Source::Local { path: input })
                .unwrap();
            let request = Request {
                source_uuid: source.uuid,
                selector: Selector::Local,
                selection: Default::default(),
                policies: Default::default(),
            };
            let mut entry = store.acquire(&request).unwrap().entry;
            let old_entry = entry.uuid;
            let app = store.register_app("recovery".into()).unwrap();
            let claim_id = Uuid::new_v4();
            let old = store
                .claim_record(
                    &claims::ClaimTarget {
                        context: Default::default(),
                        app: app.uuid,
                        id: claim_id,
                        expected: None,
                    },
                    &request,
                    old_entry,
                )
                .unwrap();
            store.save_record(&old).unwrap();
            entry.uuid = Uuid::new_v4();
            let stage = tempfile::tempdir_in(store.root.join(".staging")).unwrap();
            fs::create_dir(stage.path().join("content")).unwrap();
            fs::write(stage.path().join("content/file"), "complete").unwrap();
            let journal = Journal {
                records: vec![
                    Record::Output(Box::new(entry.clone())),
                    store
                        .claim_record(
                            &claims::ClaimTarget {
                                context: Default::default(),
                                app: app.uuid,
                                id: claim_id,
                                expected: Some(old_entry),
                            },
                            &request,
                            entry.uuid,
                        )
                        .unwrap(),
                ],
                stage: Some((
                    stage.path().file_name().unwrap().to_str().unwrap().into(),
                    entry.uuid,
                )),
                ..Default::default()
            };
            write(&store.root.join(".staging/transaction.json"), &journal).unwrap();
            if renamed {
                fs::rename(
                    stage.path().join("content"),
                    store.root.join("store").join(entry.uuid.to_string()),
                )
                .unwrap();
            }
            if corrupt {
                let content = if renamed {
                    store.root.join("store").join(entry.uuid.to_string())
                } else {
                    stage.path().join("content")
                };
                fs::write(content.join("file"), b"tampered").unwrap();
                assert!(Store::open(&store.root).is_err());
                assert_eq!(
                    store.application(app.uuid).unwrap().claims[&claim_id].entry_uuid,
                    old_entry
                );
                assert!(store.output(entry.uuid).is_err());
                continue;
            }
            let reopened = Store::open(&store.root).unwrap();
            assert_eq!(
                fs::read(reopened.path(entry.uuid).unwrap().join("file")).unwrap(),
                b"complete"
            );
            Store::open(&store.root).unwrap();
            assert_eq!(
                reopened.application(app.uuid).unwrap().claims[&claim_id].entry_uuid,
                entry.uuid
            );
            assert!(!store.root.join(".staging/transaction.json").exists());
        }
    }

    #[test]
    fn interrupted_activation_completes_an_atomic_binding() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::create(&tmp.path().join("root")).unwrap();
        let mut app = store.register_app("binding recovery".into()).unwrap();
        let input = tmp.path().join("tool");
        fs::write(&input, b"validated contents").unwrap();
        let source = store
            .register_source(Source::Local { path: input })
            .unwrap();
        let target = store
            .acquire(&Request {
                source_uuid: source.uuid,
                selector: Selector::Local,
                selection: Default::default(),
                policies: Default::default(),
            })
            .unwrap()
            .entry
            .uuid;
        app.active = Some(crate::models::index::Binding {
            entry_uuid: target,
            executable: "tool".into(),
        });
        write(
            &store.root.join(".staging/transaction.json"),
            &Journal {
                records: vec![Record::Application(app.clone())],
                stage: None,
                ..Default::default()
            },
        )
        .unwrap();
        let recovered = Store::open(&store.root).unwrap();
        assert_eq!(
            recovered
                .application(app.uuid)
                .unwrap()
                .active
                .unwrap()
                .entry_uuid,
            target
        );
        assert!(!store.root.join(".staging/transaction.json").exists());
    }
}
