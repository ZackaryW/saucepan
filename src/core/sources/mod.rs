//! Internal acquisition providers. Source transport never owns grants,
//! application bindings, presentation, or history eviction.
mod git;
mod identity;
pub(super) use git::GitPin;
pub(super) use git::GitRepository;
#[cfg(test)]
pub(super) use git::tests as git_tests;
pub(super) use identity::identify;
