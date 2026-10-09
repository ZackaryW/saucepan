//! Portable, full-string wildcard matching. No filesystem or normalization rules.

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Literal(char),
    One,
    Many,
}

/// A syntax error at a zero-based Unicode scalar position in the pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternError {
    pub position: usize,
    pub message: &'static str,
}
impl std::fmt::Display for PatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at character {}", self.message, self.position)
    }
}
impl std::error::Error for PatternError {}

/// A compiled, case-sensitive string pattern. `*` matches any number of Unicode
/// scalars and `?` matches exactly one. Backslash escapes `*?\\[]{}` only.
/// Unescaped brackets/braces and malformed escapes are errors. Repeated stars
/// collapse to one. Separators and newlines are ordinary characters.
#[derive(Debug, Clone)]
pub struct StringGlob(Vec<Token>);
impl StringGlob {
    pub fn compile(pattern: &str) -> Result<Self, PatternError> {
        let mut tokens = Vec::new();
        let mut chars = pattern.chars().enumerate();
        while let Some((position, c)) = chars.next() {
            let error = |message| PatternError { position, message };
            let token = match c {
                '*' => {
                    if tokens.last() == Some(&Token::Many) {
                        continue;
                    }
                    Token::Many
                }
                '?' => Token::One,
                '\\' => {
                    let (_, escaped) = chars.next().ok_or_else(|| error("trailing escape"))?;
                    if !"*?\\[]{}".contains(escaped) {
                        return Err(error("unsupported escape"));
                    }
                    Token::Literal(escaped)
                }
                '[' | ']' | '{' | '}' => return Err(error("unsupported glob syntax")),
                literal => Token::Literal(literal),
            };
            tokens.push(token);
        }
        Ok(Self(tokens))
    }

    /// Match the complete value with O(pattern * value) time and O(pattern)
    /// auxiliary memory. No recursion, backtracking, I/O, or persistent state.
    pub fn is_match(&self, value: &str) -> bool {
        let mut row = vec![false; self.0.len() + 1];
        row[0] = true;
        for (i, token) in self.0.iter().enumerate() {
            row[i + 1] = row[i] && *token == Token::Many;
        }
        for c in value.chars() {
            let mut diagonal = row[0];
            row[0] = false;
            for (i, token) in self.0.iter().enumerate() {
                let previous = row[i + 1];
                row[i + 1] = match token {
                    Token::Many => previous || row[i],
                    Token::One => diagonal,
                    Token::Literal(literal) => diagonal && *literal == c,
                };
                diagonal = previous;
            }
        }
        row[self.0.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_contract() {
        for (pattern, value, expected) in [
            ("", "", true),
            ("", "a", false),
            ("*", "", true),
            ("?", "", false),
            ("?", "é", true),
            ("?", "e\u{301}", false),
            ("a", "ba", false),
            ("a", "ab", false),
            ("A", "a", false),
            ("skills-*", "skills-team/core", true),
            ("*", "\\/\n", true),
            (r"C:\\work\\*", r"C:\work\file", true),
            (r"C:\\work\\*", "C:/work/file", false),
            (r"\*\?\[\]\{\}", "*?[]{}", true),
            ("a***?b", "aéb", true),
            ("*a*b", "abx", false),
        ] {
            let glob = StringGlob::compile(pattern).unwrap();
            assert_eq!(glob.is_match(value), expected, "{pattern:?} {value:?}");
            assert_eq!(glob.is_match(value), expected);
        }
        for pattern in ["[ab]", "{a,b}", "a\\", r"\a", "]", "}"] {
            assert!(StringGlob::compile(pattern).is_err(), "{pattern}");
        }
        assert_eq!(StringGlob::compile("é[").unwrap_err().position, 1);
        assert!(
            !StringGlob::compile(&"*a".repeat(256))
                .unwrap()
                .is_match(&"b".repeat(4096))
        );
    }

    #[test]
    fn generated_cases_match_independent_recursive_reference() {
        fn reference(pattern: &[char], value: &[char]) -> bool {
            match pattern.split_first() {
                None => value.is_empty(),
                Some(('*', rest)) => (0..=value.len()).any(|n| reference(rest, &value[n..])),
                Some((c, rest)) => {
                    !value.is_empty()
                        && (*c == '?' || *c == value[0])
                        && reference(rest, &value[1..])
                }
            }
        }
        fn words(alphabet: &[char], length: usize) -> Vec<String> {
            let mut out = vec![String::new()];
            let mut level = out.clone();
            for _ in 0..length {
                level = level
                    .iter()
                    .flat_map(|s| alphabet.iter().map(move |c| format!("{s}{c}")))
                    .collect();
                out.extend(level.clone());
            }
            out
        }
        for pattern in words(&['a', 'é', '*', '?'], 4) {
            let glob = StringGlob::compile(&pattern).unwrap();
            for value in words(&['a', 'é', '/'], 3) {
                assert_eq!(
                    glob.is_match(&value),
                    reference(
                        &pattern.chars().collect::<Vec<_>>(),
                        &value.chars().collect::<Vec<_>>()
                    ),
                    "{pattern:?} {value:?}"
                );
            }
        }
    }
}
