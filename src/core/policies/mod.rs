use super::models::{Filters, Source};

/// Select records for an app's view without granting or denying acquisition.
pub fn selected(filters: &Filters, source_id: &str, source: &Source) -> bool {
    (filters.source_ids.is_empty() || filters.source_ids.contains(source_id))
        && (filters.providers.is_empty() || filters.providers.contains(source.provider()))
}

/// Return the ZIP IDs evicted by a committed transition. Content has its own lifetime.
pub(crate) fn advance(
    state: &mut super::models::SourceState,
    incoming: super::models::Snapshot,
    retain: bool,
) -> Vec<String> {
    let mut evicted = Vec::new();
    if state
        .current
        .as_ref()
        .is_some_and(|old| old.id == incoming.id)
    {
        state.current = Some(incoming);
        return evicted;
    }
    if retain {
        state.history.retain(|snapshot| snapshot.id != incoming.id);
        if let Some(outgoing) = state.current.take().filter(|old| old.zip_digest.is_some()) {
            state.history.push(outgoing);
        }
        evicted = trim_history(state);
    }
    state.current = Some(incoming);
    evicted
}

pub(crate) fn trim_history(state: &mut super::models::SourceState) -> Vec<String> {
    let mut evicted = Vec::new();
    while state.history.len() > 5 {
        let oldest = state
            .history
            .iter()
            .enumerate()
            .min_by_key(|(_, s)| (s.last_used, s.created, &s.id))
            .map(|(i, _)| i)
            .unwrap();
        evicted.push(state.history.remove(oldest).id);
    }
    evicted
}
