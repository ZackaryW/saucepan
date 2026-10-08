use crate::{
    core::{Store, index::Journal},
    models::{
        index::Record,
        policy::Eviction,
        store::{Availability, ReclaimReport},
    },
    utils::{
        lock::{LockGuard, LockMode},
        path::beneath,
    },
};
use anyhow::Result;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    time::Duration,
};
use uuid::Uuid;
impl Store {
    pub fn reclaim(&self) -> Result<ReclaimReport> {
        let _lock = self.lock()?;
        self.reclaim_locked()
    }
    pub(crate) fn reclaim_locked(&self) -> Result<ReclaimReport> {
        // Bookkeeping cleanup is independent of whether payloads are evictable.
        self.sweep_closed_markers_locked()?;
        let records = self.records()?;
        let mut groups: BTreeMap<String, Vec<_>> = BTreeMap::new();
        let mut active = BTreeSet::new();
        let mut claimed_keep: BTreeMap<String, usize> = BTreeMap::new();
        for record in records {
            match record {
                Record::Output(e) => {
                    groups.entry(e.item_key.clone()).or_default().push(*e);
                }
                Record::Application(app) => {
                    for claim in app.claims.values() {
                        let request = &claim.request;
                        let key = super::json_digest(&(
                            request.source_uuid,
                            &request.selector,
                            &request.selection,
                        ))?;
                        let Eviction::Latest { keep } = request.policies.eviction;
                        claimed_keep
                            .entry(key)
                            .and_modify(|n| *n = (*n).max(keep))
                            .or_insert(keep);
                    }
                    if let Some(binding) = app.active {
                        active.insert(binding.entry_uuid);
                    }
                }
            }
        }
        let mut report = ReclaimReport::default();
        for (key, mut entries) in groups {
            entries.sort_by_key(|e| std::cmp::Reverse(e.sequence));
            let keep = entries
                .iter()
                .filter(|e| e.state != Availability::Reclaimed)
                .map(|e| {
                    let Eviction::Latest { keep } = e.request.policies.eviction;
                    keep
                })
                .max()
                .max(claimed_keep.get(&key).copied())
                .unwrap_or(10);
            report.effective_keep.insert(key, keep);
            for (rank, mut entry) in entries.into_iter().enumerate() {
                if entry.state == Availability::Reclaimed
                    || (rank < keep && entry.state == Availability::Ready)
                {
                    continue;
                }
                if active.contains(&entry.uuid) && entry.request.policies.protection.is_some() {
                    report
                        .deferred
                        .insert(entry.uuid, "active useable_app binding".into());
                    continue;
                }
                let lease = LockGuard::acquire(
                    &self.root.join(format!(".locks/use-{}", entry.uuid)),
                    LockMode::Exclusive,
                    Duration::ZERO,
                );
                let _lease = match lease {
                    Ok(lease) => lease,
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                        report
                            .deferred
                            .insert(entry.uuid, "live execution lease".into());
                        continue;
                    }
                    Err(e) => return Err(e.into()),
                };
                if self.has_live_execution(entry.uuid)? {
                    report
                        .deferred
                        .insert(entry.uuid, "inherited execution lifetime".into());
                    continue;
                }
                entry.state = Availability::ReclaimPending;
                self.commit(Journal {
                    records: vec![Record::Output(Box::new(entry.clone()))],
                    stage: None,
                })?;
                let path = beneath(&self.root, &format!("store/{}", entry.uuid))?;
                match fs::remove_dir_all(&path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        report.pending.insert(entry.uuid, e.to_string());
                        continue;
                    }
                }
                entry.state = Availability::Reclaimed;
                report.reclaimed.push(entry.uuid);
                self.commit(Journal {
                    records: vec![Record::Output(Box::new(entry))],
                    stage: None,
                })?;
            }
        }
        Ok(report)
    }
    pub fn reacquire(
        &self,
        entry: Uuid,
        options: &super::super::source::AcquireOptions,
    ) -> Result<crate::models::store::Acquired> {
        self.acquire_with(&self.output(entry)?.request, options)
    }
}
