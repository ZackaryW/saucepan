//! Stateless helpers used by the new core, independent of legacy utilities.

pub(crate) fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 15) as usize] as char);
    }
    out
}

pub(crate) fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let a = (pair[0] as char).to_digit(16)?;
            let b = (pair[1] as char).to_digit(16)?;
            Some(((a << 4) | b) as u8)
        })
        .collect()
}

/// Encode an ordered tuple with explicit byte lengths rather than delimiters.
pub(crate) fn frame(domain: &[u8], fields: &[&[u8]]) -> Vec<u8> {
    let mut result = Vec::new();
    for field in std::iter::once(&domain).chain(fields.iter()) {
        result.extend_from_slice(&(field.len() as u64).to_be_bytes());
        result.extend_from_slice(field);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn component_boundaries_cannot_collide() {
        assert_ne!(frame(b"v1", &[b"a/b", b"c"]), frame(b"v1", &[b"a", b"b/c"]));
        assert_ne!(frame(b"source", &[b"a"]), frame(b"artifact", &[b"a"]));
    }
}
