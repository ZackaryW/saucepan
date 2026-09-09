//! Evaluates explicit authority and source facts; never reads configuration.
use super::models::{CachePolicy, EffectivePolicy, Error, ErrorKind, Result, VerificationRequest};

pub(super) fn evaluate(
    source: &CachePolicy,
    authority_requires: bool,
    request: &VerificationRequest,
) -> Result<EffectivePolicy> {
    let required = source.require_verification || authority_requires;
    if required && request.content == Some(false) {
        return Err(Error::new(
            ErrorKind::Authority,
            "content verification is required by authority",
        ));
    }
    Ok(EffectivePolicy {
        revision: source.revision,
        retain: source.enabled,
        history_limit: source.history_limit,
        verify_content: required || request.content.unwrap_or(source.verify_by_default),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_authority_floor_are_independent_of_retention() {
        let mut source = CachePolicy::default();
        let request = VerificationRequest { content: None };
        let p = evaluate(&source, false, &request).unwrap();
        assert!(p.retain);
        assert_eq!(p.history_limit, 5);
        assert!(!p.verify_content);
        source.enabled = false;
        assert!(evaluate(&source, true, &request).unwrap().verify_content);
        assert_eq!(
            evaluate(
                &source,
                true,
                &VerificationRequest {
                    content: Some(false)
                }
            )
            .unwrap_err()
            .kind,
            ErrorKind::Authority
        );
    }
}
