//! OS keyring custody and authenticated, recoverable persistence.
//! Store mutation is not part of the public library API.
mod crypto;
mod keyring;
mod layout;
mod locking;
mod mirrors;
mod session;
mod transactions;
pub(super) use session::Session;
