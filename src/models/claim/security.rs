use crate::error::{Error, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::Zeroizing;

/// Secret material deliberately has no Serialize, Deserialize, Clone, or revealing Debug.
pub struct Credential(Zeroizing<[u8; 32]>);
impl Credential {
    pub fn generate() -> Result<Self> {
        crate::utils::crypto::random_bytes()
            .map(Self)
            .map_err(Error::from)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut out = Zeroizing::new([0u8; 32]);
        if bytes.len() != out.len() {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "invalid credential length",
            ));
        }
        out.copy_from_slice(bytes);
        Ok(Self(out))
    }
    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}
impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Credential([REDACTED])")
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum CredentialPolicy {
    #[default]
    Disabled,
    Required {
        generation: u64,
        verifier: [u8; 32],
    },
}
impl fmt::Debug for CredentialPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str("Disabled"),
            Self::Required { generation, .. } => f
                .debug_struct("Required")
                .field("generation", generation)
                .field("verifier", &"[REDACTED]")
                .finish(),
        }
    }
}
