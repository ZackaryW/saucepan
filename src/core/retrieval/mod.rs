//! Source-material ownership, separate from acquired-output ownership.
use crate::{models::retrieval::RetrievedDescriptor, utils::lock::LockGuard};
use std::{path::Path, rc::Rc};

/// A source path and its native lifetime. Only retrieval operations may construct
/// this handle; deserializing a descriptor cannot establish coordinated access.
/// The handle is intentionally neither serializable nor cloneable.
pub struct RetrievedResource {
    pub(crate) descriptor: RetrievedDescriptor,
    pub(crate) _temporary: Option<Rc<tempfile::TempDir>>,
    pub(crate) _coordination: Option<LockGuard>,
}
impl RetrievedResource {
    pub fn source_path(&self) -> &Path {
        &self.descriptor.path
    }

    /// Observational evidence only; retaining this value does not retain a lease.
    pub fn descriptor(&self) -> &RetrievedDescriptor {
        &self.descriptor
    }
}
