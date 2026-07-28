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
        assert_eq!(
            terminal_component("https://example.com/owner/repo"),
            "repo"
        );
    }

    #[test]
    fn terminal_component_of_plain_name_is_unchanged() {
        assert_eq!(terminal_component("my-package"), "my-package");
    }

    #[test]
    fn terminal_component_with_trailing_separator_is_empty() {
        assert_eq!(
            terminal_component("https://example.com/owner/repo/"),
            ""
        );
    }
}
