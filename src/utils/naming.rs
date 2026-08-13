/// Extract the final forward-slash-delimited component of a target string.
///
/// Used to derive a terminal name from a custom-Git URL or slug, e.g.
/// `https://example.com/owner/repo` -> `repo`.
pub fn terminal_component(s: &str) -> &str {
    s.rsplit('/').next().unwrap_or(s)
}

/// Convert a logical repo name to a safe on-disk directory name.
///
/// Uses `--` as separator for `/` so that `owner/repo` → `owner--repo`
/// and `owner_repo` → `owner_repo` remain distinct (injective for typical names).
pub fn repo_dir(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        match c {
            '/' => out.push_str("--"),
            _ if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' => out.push(c),
            _ => out.push('_'),
        }
    }
    out
}

/// Canonicalize a repository-target string for **comparison purposes only**.
///
/// Used by the central-index manifest link (`src/sources/git.rs`) to decide
/// whether a registered index's stub `url` names the same repository as the
/// target being installed, even when the two spellings differ trivially
/// (a `.git` suffix, a trailing separator, `\` vs `/`, or one of the three
/// equivalent GitHub spellings). It never touches anything stored or
/// displayed — callers keep using the original strings for that; only the
/// return value of this function is ever compared against another.
///
/// Case is folded **only** for targets recognized as GitHub-shaped (a bare
/// `owner/repo` slug, an `https://github.com/owner/repo` URL, or a
/// `git@github.com:owner/repo` SSH URL), because `github.com` itself treats
/// `owner/repo` case-insensitively. Anything else — a local filesystem path,
/// a custom Git host's URL — is left with its case untouched: those may
/// live on a case-sensitive filesystem or a case-sensitive Git server, so
/// folding their case could make two genuinely different repositories
/// compare equal. A missed match (falling through to `NotFound`, today's
/// behavior) is far safer than a wrong one.
pub fn normalize_target(target: &str) -> String {
    // Backslash-vs-forward-slash is purely a Windows local-path spelling
    // difference; normalize it before anything else so later checks only
    // need to think about `/`.
    let mut s = target.replace('\\', "/");

    // A single trailing separator never changes what's being named.
    if let Some(stripped) = s.strip_suffix('/') {
        s = stripped.to_string();
    }

    if let Some(slug) = github_owner_repo(&s) {
        return slug;
    }

    // Not recognized as GitHub-shaped: preserve case, only drop a trailing
    // `.git` suffix (case-sensitive — `.git` is a Git convention, not a
    // case-insensitive one for arbitrary hosts).
    if let Some(stripped) = s.strip_suffix(".git") {
        s = stripped.to_string();
    }
    s
}

/// If `s` (already slash-normalized, trailing separator already stripped) is
/// one of the three GitHub-shaped spellings of an `owner/repo` target,
/// return the canonical, lower-cased `owner/repo` form. Otherwise `None` —
/// the caller must leave the string's case alone.
fn github_owner_repo(s: &str) -> Option<String> {
    let rest =
        strip_ci_prefix(s, "https://github.com/").or_else(|| strip_ci_prefix(s, "git@github.com:"));

    let owner_repo = match rest {
        Some(rest) => rest,
        None => {
            // Bare slug candidate. Anything carrying a scheme/auth marker
            // is some other (possibly case-sensitive) URL shape, not a
            // plain `owner/repo` slug — leave it alone.
            if s.contains(['@', ':']) || s.contains("//") {
                return None;
            }
            s
        }
    };
    let owner_repo = owner_repo.strip_suffix(".git").unwrap_or(owner_repo);

    let mut parts = owner_repo.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if parts.next().is_some() {
        return None; // more than two segments — not a bare `owner/repo`
    }
    let is_dot = |p: &str| p.is_empty() || p == "." || p == "..";
    if is_dot(owner) || is_dot(repo) {
        return None;
    }

    Some(format!("{}/{}", owner.to_lowercase(), repo.to_lowercase()))
}

/// Like `str::strip_prefix`, but ASCII case-insensitive — used for the
/// `https://github.com/` and `git@github.com:` prefixes, since scheme and
/// host are conventionally case-insensitive.
fn strip_ci_prefix<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let candidate = s.get(..prefix.len())?;
    candidate
        .eq_ignore_ascii_case(prefix)
        .then(|| &s[prefix.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_becomes_double_dash() {
        assert_eq!(repo_dir("owner/repo"), "owner--repo");
    }

    #[test]
    fn underscore_name_is_unchanged() {
        assert_eq!(repo_dir("owner_repo"), "owner_repo");
    }

    #[test]
    fn underscore_and_slash_are_distinct() {
        assert_ne!(repo_dir("owner/repo"), repo_dir("owner_repo"));
    }

    #[test]
    fn hyphens_preserved() {
        assert_eq!(repo_dir("my-org/my-repo"), "my-org--my-repo");
    }

    #[test]
    fn plain_name_unchanged() {
        assert_eq!(repo_dir("mylib"), "mylib");
    }

    #[test]
    fn terminal_component_extracts_final_segment() {
        assert_eq!(terminal_component("https://example.com/owner/repo"), "repo");
    }

    #[test]
    fn terminal_component_of_plain_name_is_unchanged() {
        assert_eq!(terminal_component("my-package"), "my-package");
    }

    #[test]
    fn terminal_component_with_trailing_separator_is_empty() {
        assert_eq!(terminal_component("https://example.com/owner/repo/"), "");
    }

    #[test]
    fn normalize_target_strips_trailing_dot_git() {
        assert_eq!(
            normalize_target("https://example.com/owner/repo.git"),
            "https://example.com/owner/repo"
        );
    }

    #[test]
    fn normalize_target_strips_trailing_slash() {
        assert_eq!(
            normalize_target("/srv/git/repo/"),
            normalize_target("/srv/git/repo")
        );
    }

    #[test]
    fn normalize_target_treats_backslash_and_forward_slash_as_equivalent() {
        assert_eq!(
            normalize_target(r"C:\Users\zack\Repo"),
            normalize_target("C:/Users/zack/Repo")
        );
    }

    #[test]
    fn normalize_target_equates_the_three_github_spellings() {
        let slug = normalize_target("owner/repo");
        assert_eq!(slug, normalize_target("https://github.com/owner/repo.git"));
        assert_eq!(slug, normalize_target("git@github.com:owner/repo.git"));
    }

    #[test]
    fn normalize_target_folds_case_for_github_shaped_targets() {
        assert_eq!(
            normalize_target("Owner/Widget"),
            normalize_target("owner/widget")
        );
        assert_eq!(
            normalize_target("https://github.com/Owner/Widget"),
            normalize_target("owner/widget")
        );
        assert_eq!(
            normalize_target("git@github.com:Owner/Widget.git"),
            normalize_target("owner/widget")
        );
    }

    /// Negative case, deliberate: unlike the GitHub-shaped forms above, a
    /// target not recognized as GitHub-shaped must NOT have its case
    /// folded. Two local filesystem paths differing only in case name
    /// different files on a case-sensitive filesystem, so treating them as
    /// the same target would be a false, and worse, match than the missed
    /// match this function is meant to fix.
    #[test]
    fn normalize_target_does_not_fold_case_for_a_non_github_local_path() {
        assert_ne!(
            normalize_target("/srv/git/Owner/Widget"),
            normalize_target("/srv/git/owner/widget")
        );
    }

    /// Negative case: a custom (non-GitHub) SSH remote is not recognized as
    /// GitHub-shaped, so its case must be preserved rather than folded the
    /// way the `git@github.com:` form is.
    #[test]
    fn normalize_target_does_not_fold_case_for_a_custom_git_ssh_host() {
        assert_ne!(
            normalize_target("git@example.com:Org/Repo.git"),
            normalize_target("git@example.com:org/repo.git")
        );
    }
}
