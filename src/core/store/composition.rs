use super::access::ResourceLease;
use crate::{core::retrieval::RetrievedResource, models::store::AcquiredDescriptor};
use std::{path::Path, rc::Rc};

/// Copied files with explicit ownership. A native managed lease or temporary
/// owner stays alive with the result; descriptors alone do not own either.
pub struct AcquiredResource {
    pub(crate) descriptor: AcquiredDescriptor,
    pub(crate) _temporary: Option<Rc<tempfile::TempDir>>,
    pub(crate) _lease: Option<ResourceLease>,
    pub(crate) _retrieval: Option<RetrievedResource>,
}
impl AcquiredResource {
    pub fn path(&self) -> &Path {
        &self.descriptor.path
    }

    pub fn descriptor(&self) -> &AcquiredDescriptor {
        &self.descriptor
    }
}
