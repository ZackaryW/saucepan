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
}
