//! Read only caller-owned, currently inspectable metadata. Native providers
//! receive the filtered source set, never another application's bindings.
use super::*;
use crate::core::{authority, sources};
use std::collections::BTreeSet;

impl Session {
    pub(in crate::core) fn application_view(
        &self,
        root: &Path,
        marker: Option<&Marker>,
    ) -> Result<ApplicationView> {
        self.project_units(root, marker, None, true)
    }

    pub(in crate::core) fn application_generation(
        &self,
        root: &Path,
        marker: Option<&Marker>,
    ) -> Result<ApplicationGeneration> {
        self.project_units(root, marker, None, false)
            .map(|view| view.generation)
    }

    pub(in crate::core) fn unit_view(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        name: &str,
    ) -> Result<UnitView> {
        self.project_units(root, marker, Some(name), true)?
            .units
            .into_iter()
            .next()
            .ok_or_else(Error::not_found)
    }

    fn project_units(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        name: Option<&str>,
        include_units: bool,
    ) -> Result<ApplicationView> {
        let context = self.inspect_context(root, marker)?;
        let deadline = Instant::now() + Duration::from_secs(5);
        let _binding = Lock::acquire(&self.layout, &format!("binding-{}", context.id), deadline)?;
        let central = self.read_index()?;
        let context = self.context_in(&central, root, marker)?;
        let mut visible: Vec<_> = central
            .units
            .values()
            .filter(|unit| unit.app_id == context.id && name.is_none_or(|name| unit.name == name))
            .filter(|unit| inspectable(&context, unit))
            .collect();
        visible.sort_by(|a, b| a.name.cmp(&b.name));
        if name.is_some() && visible.is_empty() {
            return Err(Error::not_found());
        }
        let mut ids = BTreeSet::new();
        for unit in &visible {
            ids.insert(unit.artifact.inputs.source_id.clone());
            ids.extend(
                unit.artifact
                    .inputs
                    .dependencies
                    .iter()
                    .map(|d| d.source_id.clone()),
            );
            ids.extend(catalog_sources(unit).map(str::to_owned));
        }
        let _sources: Vec<_> = ids
            .iter()
            .map(|id| Lock::acquire(&self.layout, &format!("source-{id}"), deadline))
            .collect::<Result<_>>()?;
        let snapshot = {
            let _central = Lock::acquire(&self.layout, "central", deadline)?;
            self.generations().recover()?;
            self.generations().read()?
        };
        self.revalidate_projection(&context, &snapshot.central, root, marker)?;
        // Origin checks are local native Git probes; no fetch or ZIP read is
        // needed to return installed metadata or check a cached view's token.
        for id in &ids {
            let source = snapshot.sources.get(id).ok_or_else(|| {
                Error::new(
                    ErrorKind::Integrity,
                    "visible source is missing from the encrypted index",
                )
            })?;
            let identity = sources::recorded_git(id, &source.origin)?;
            let repo = self
                .layout
                .path(&Path::new("sources").join(id).join("repo.git"))?;
            sources::GitRepository::open(&repo, &identity)?;
        }
        for unit in &visible {
            let identity = sources::identify(&unit.recipe.source, Path::new(&context.root))?;
            if identity.id != unit.artifact.inputs.source_id
                || snapshot.sources[&identity.id].origin != identity.origin
                || unit.artifact.inputs.dependencies.iter().any(|dependency| {
                    snapshot.sources[&dependency.source_id].origin != dependency.origin
                })
            {
                return Err(Error::new(
                    ErrorKind::Integrity,
                    "binding source provenance changed",
                ));
            }
        }
        let _central = Lock::acquire(
            &self.layout,
            "central",
            Instant::now() + Duration::from_secs(5),
        )?;
        self.generations().recover()?;
        let selected = self.generations().read()?;
        self.revalidate_projection(&context, &selected.central, root, marker)?;
        if ids
            .iter()
            .any(|id| selected.central.sources.get(id) != snapshot.central.sources.get(id))
        {
            return Err(Error::new(
                ErrorKind::Busy,
                "visible sources changed during lookup",
            ));
        }
        Ok(ApplicationView {
            schema_version: SCHEMA_VERSION,
            generation: self
                .keys
                .application_generation(&context, selected.central.key_generation)?,
            units: if include_units {
                visible
                    .into_iter()
                    .map(|unit| UnitView {
                        id: unit.id.clone(),
                        name: unit.name.clone(),
                        recipe: unit.recipe.clone(),
                        artifact: unit.artifact.clone(),
                        manifest: unit.manifest.clone(),
                    })
                    .collect()
            } else {
                vec![]
            },
        })
    }

    fn revalidate_projection(
        &self,
        expected: &Registration,
        central: &CentralIndex,
        root: &Path,
        marker: Option<&Marker>,
    ) -> Result<()> {
        let current = self.context_in(central, root, marker)?;
        if current.id != expected.id || current.revision != expected.revision {
            return Err(Error::new(
                ErrorKind::Authority,
                "application scope changed during lookup",
            ));
        }
        if current.data_generation != expected.data_generation {
            return Err(Error::new(
                ErrorKind::Busy,
                "application data changed during lookup",
            ));
        }
        Ok(())
    }
}

fn inspectable(context: &Registration, unit: &UnitBinding) -> bool {
    let inputs = &unit.artifact.inputs;
    authority::require_source(
        context,
        &inputs.source_id,
        &inputs.subdirectory,
        Action::Inspect,
    )
    .is_ok()
        && inputs.dependencies.iter().all(|dependency| {
            authority::require_source(
                context,
                &dependency.source_id,
                &dependency.subdirectory,
                Action::Inspect,
            )
            .is_ok()
        })
        && catalog_sources(unit)
            .all(|id| authority::require_source(context, id, ".", Action::Inspect).is_ok())
}

fn catalog_sources(unit: &UnitBinding) -> impl Iterator<Item = &str> {
    [
        unit.recipe.manifest.as_ref(),
        unit.artifact.evidence.manifest.as_ref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(|input| match &input.provenance {
        ManifestProvenance::Catalog { source_id, .. } => Some(source_id.as_str()),
        _ => None,
    })
}
