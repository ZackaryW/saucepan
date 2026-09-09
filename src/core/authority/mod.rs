//! Current application context and internal operation capabilities.
//! No caller-supplied boolean can represent authorization.
use super::models::{Action, Error, ErrorKind, Registration, Result};

pub(super) fn path_contains(root: &std::path::Path, path: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        fn spelling(path: &std::path::Path) -> String {
            let value = path.to_string_lossy().to_lowercase();
            if let Some(unc) = value.strip_prefix("\\\\?\\unc\\") {
                format!("\\\\{unc}")
            } else {
                value.strip_prefix("\\\\?\\").unwrap_or(&value).into()
            }
        }
        std::path::Path::new(&spelling(path)).starts_with(std::path::Path::new(&spelling(root)))
    }
    #[cfg(not(windows))]
    {
        path.starts_with(root)
    }
}

pub(super) fn require_destination(
    registration: &Registration,
    source: &str,
    selection: &str,
    action: Action,
    destination: &std::path::Path,
) -> Result<()> {
    require_source(registration, source, selection, action)?;
    if !registration.grants.iter().any(|grant| {
        grant.source_id.as_deref() == Some(source)
            && grant.actions.contains(&action)
            && (grant.selection == "."
                || selection == grant.selection
                || selection
                    .strip_prefix(&grant.selection)
                    .is_some_and(|suffix| suffix.starts_with('/')))
            && grant
                .destinations
                .iter()
                .any(|root| path_contains(std::path::Path::new(root), destination))
    }) {
        return Err(Error::new(
            ErrorKind::Authority,
            "destination is outside the application's scope",
        ));
    }
    Ok(())
}

pub(super) fn require_source(
    registration: &Registration,
    id: &str,
    selection: &str,
    action: Action,
) -> Result<()> {
    super::recipes::validate_selection(selection)?;
    if registration.revoked
        || !registration.grants.iter().any(|grant| {
            grant.source_id.as_deref() == Some(id)
                && grant.actions.contains(&action)
                && (grant.selection == "."
                    || selection == grant.selection
                    || selection
                        .strip_prefix(&grant.selection)
                        .is_some_and(|rest| rest.starts_with('/')))
        })
    {
        return Err(if action == Action::Inspect {
            Error::not_found()
        } else {
            Error::new(
                ErrorKind::Authority,
                "source operation is outside the application's scope",
            )
        });
    }
    Ok(())
}
