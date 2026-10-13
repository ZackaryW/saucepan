//! Source-material ownership, separate from acquired-output ownership.
use crate::{
    models::retrieval::{RetrieveRequest, RetrievedDescriptor},
    utils::lock::LockGuard,
};
use std::{path::Path, rc::Rc};
mod access;
mod caller;
mod fresh;
mod managed;
mod registered;
pub use registered::{RegisteredAppRetriever, RegisteredRetriever};
#[cfg(test)]
mod tests;
pub(crate) use fresh::delivery_lock;
pub use fresh::{RetrieveOptions, Retriever};

/// A source path and its native lifetime. Only retrieval operations may construct
/// this handle; deserializing a descriptor cannot establish coordinated access.
/// The handle is intentionally neither serializable nor cloneable.
pub struct RetrievedResource {
    pub(crate) request: RetrieveRequest,
    pub(crate) source: crate::models::source::Source,
    pub(crate) store: Option<crate::core::Store>,
    pub(crate) authorization: Option<crate::core::claim::composition::Authorization>,
    pub(crate) descriptor: RetrievedDescriptor,
    pub(crate) _temporary: Option<Rc<tempfile::TempDir>>,
    pub(crate) _coordination: Option<LockGuard>,
}
impl RetrievedResource {
    pub fn request(&self) -> &RetrieveRequest {
        &self.request
    }

    pub fn source_path(&self) -> &Path {
        &self.descriptor.path
    }

    /// Observational evidence only; retaining this value does not retain a lease.
    pub fn descriptor(&self) -> &RetrievedDescriptor {
        &self.descriptor
    }
}
