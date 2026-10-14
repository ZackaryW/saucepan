//! Named requests and informational descriptors; ownership stays in core::lease.
mod group;
pub use group::{GroupDescriptor, GroupMember, GroupMemberDescriptor, GroupRequest, GroupTarget};
