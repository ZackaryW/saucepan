//! Current artifacts and the source-owned historical pool.
//! Installed units do not own retention queues.
mod export;
use super::models::*;
pub(super) use export::prepare_export;
pub(super) use export::validate_lfs;
pub(super) use export::{Exported, PreparedExport};
pub(super) use export::{decode, tree_digest};

pub(super) fn prepare(
    source: &mut SourceIndex,
    stream_id: &str,
    recipe: StreamDescriptor,
    artifact: ContentArtifact,
    retain: bool,
) -> Result<()> {
    let next = source
        .sequence
        .checked_add(1)
        .ok_or_else(|| Error::new(ErrorKind::Integrity, "source recency exhausted"))?;
    if retain
        && let Some(previous) = source.current.get(stream_id)
        && previous.artifact.id != artifact.id
    {
        let id = previous.artifact.id.clone();
        let historical = source.history.entry(id).or_insert_with(|| Historical {
            artifact: previous.artifact.clone(),
            created: next,
            last_used: next,
        });
        historical.last_used = next;
    }
    let generation = source
        .current
        .get(stream_id)
        .map_or(0, |c| c.generation)
        .checked_add(1)
        .ok_or_else(|| Error::new(ErrorKind::Integrity, "stream generation exhausted"))?;
    source.current.insert(
        stream_id.into(),
        Current {
            recipe,
            artifact,
            generation,
            last_used: next,
            archive_retained: retain,
        },
    );
    source.sequence = next;
    Ok(())
}

#[cfg(test)]
pub(super) fn advance(
    source: &mut SourceIndex,
    stream_id: &str,
    recipe: StreamDescriptor,
    artifact: ContentArtifact,
    retain: bool,
) -> Result<()> {
    prepare(source, stream_id, recipe, artifact, retain)?;
    while source.history.len() > source.policy.history_limit {
        let victim = source
            .history
            .iter()
            .min_by_key(|(id, h)| (h.last_used, h.created, *id))
            .map(|(id, _)| id.clone())
            .unwrap();
        source.history.remove(&victim);
    }
    Ok(())
}
