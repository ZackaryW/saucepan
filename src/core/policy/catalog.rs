//! Bounded in-memory lookup. Persistence and authorization wrap this catalog.
use crate::{
    error::{Error, ErrorCode, Result},
    models::policy::PolicyRecord,
};
use std::collections::BTreeMap;
use uuid::Uuid;

const MAX_CATALOG_RECORDS: usize = 4096;

#[derive(Debug, Clone, Default)]
pub struct PolicyCatalog {
    records: BTreeMap<Uuid, PolicyRecord>,
}
impl PolicyCatalog {
    pub fn new(records: impl IntoIterator<Item = PolicyRecord>) -> Result<Self> {
        let mut catalog = Self::default();
        for record in records {
            record.definition.validate()?;
            if catalog.records.len() >= MAX_CATALOG_RECORDS
                || catalog.records.contains_key(&record.uuid)
            {
                return Err(Error::new(
                    ErrorCode::InvalidRequest,
                    "duplicate or excessive catalog definitions",
                ));
            }
            catalog.records.insert(record.uuid, record);
        }
        Ok(catalog)
    }
    pub fn get(&self, uuid: Uuid) -> Result<&PolicyRecord> {
        self.records
            .get(&uuid)
            .ok_or_else(|| Error::new(ErrorCode::InvalidRequest, "unknown policy UUID"))
    }
    pub fn records(&self) -> impl Iterator<Item = &PolicyRecord> {
        self.records.values()
    }
    /// Portable definitions cannot replace or alias a persisted definition.
    pub fn with_portable(&self, portable: &Self) -> Result<Self> {
        Self::new(self.records().chain(portable.records()).cloned())
    }
}
