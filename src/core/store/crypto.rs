use crate::core::models::{
    DocumentIdentity, EncryptedEnvelope, Error, ErrorKind, Marker, MarkerClaims, Result,
    SCHEMA_VERSION,
};
use crate::utils::frame;
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub(super) const MAX_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;
pub(super) const MAX_ENVELOPE_BYTES: usize = 48 * 1024 * 1024;

#[derive(Zeroize, ZeroizeOnDrop)]
pub(super) struct Keys {
    encryption: [u8; 32],
    authentication: [u8; 32],
}

impl Keys {
    pub(super) fn derive(master: &[u8; 32], store_id: &str) -> Result<Self> {
        let hkdf = Hkdf::<Sha256>::new(Some(store_id.as_bytes()), master);
        let mut keys = Self {
            encryption: [0; 32],
            authentication: [0; 32],
        };
        hkdf.expand(b"saucepan/index-encryption/v1", &mut keys.encryption)
            .map_err(|_| Error::new(ErrorKind::Internal, "key derivation failed"))?;
        hkdf.expand(b"saucepan/registration-mac/v1", &mut keys.authentication)
            .map_err(|_| Error::new(ErrorKind::Internal, "key derivation failed"))?;
        Ok(keys)
    }

    pub(super) fn seal(&self, identity: &DocumentIdentity, plaintext: &[u8]) -> Result<Vec<u8>> {
        validate_identity(identity)?;
        if plaintext.len() > MAX_DOCUMENT_BYTES {
            return Err(integrity("index exceeds size limit"));
        }
        let mut nonce = [0; 24];
        getrandom::fill(&mut nonce)
            .map_err(|_| Error::new(ErrorKind::Internal, "OS random source unavailable"))?;
        self.seal_with_nonce(identity, plaintext, nonce)
    }

    fn seal_with_nonce(
        &self,
        identity: &DocumentIdentity,
        plaintext: &[u8],
        nonce: [u8; 24],
    ) -> Result<Vec<u8>> {
        let cipher = XChaCha20Poly1305::new((&self.encryption).into());
        let aad = identity_bytes(identity);
        let ciphertext = cipher
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| integrity("index encryption failed"))?;
        serde_json::to_vec(&EncryptedEnvelope {
            identity: identity.clone(),
            nonce: nonce.to_vec(),
            ciphertext,
        })
        .map_err(|_| Error::new(ErrorKind::Internal, "cannot encode encrypted index"))
    }

    pub(super) fn open(
        &self,
        expected: &DocumentIdentity,
        bytes: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>> {
        validate_identity(expected)?;
        if bytes.len() > MAX_ENVELOPE_BYTES {
            return Err(integrity("encrypted index exceeds size limit"));
        }
        let envelope: EncryptedEnvelope = serde_json::from_slice(bytes)
            .map_err(|_| integrity("invalid encrypted index envelope"))?;
        validate_identity(&envelope.identity)?;
        if &envelope.identity != expected || envelope.ciphertext.len() > MAX_DOCUMENT_BYTES + 16 {
            return Err(integrity("encrypted index identity or size mismatch"));
        }
        let nonce: [u8; 24] = envelope
            .nonce
            .try_into()
            .map_err(|_| integrity("invalid encrypted index nonce"))?;
        let cipher = XChaCha20Poly1305::new((&self.encryption).into());
        let aad = identity_bytes(expected);
        let plaintext = cipher
            .decrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: &envelope.ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| integrity("encrypted index authentication failed"))?;
        Ok(Zeroizing::new(plaintext))
    }

    pub(super) fn sign(&self, claims: MarkerClaims) -> Result<Marker> {
        if claims.schema_version != SCHEMA_VERSION {
            return Err(integrity("invalid registration version"));
        }
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(&self.authentication)
            .map_err(|_| Error::new(ErrorKind::Internal, "invalid MAC key"))?;
        mac.update(&claim_bytes(&claims));
        Ok(Marker {
            claims,
            mac: crate::utils::hex(&mac.finalize().into_bytes()),
        })
    }

    pub(super) fn verify(&self, marker: &Marker) -> Result<()> {
        if marker.claims.schema_version != SCHEMA_VERSION {
            return Err(integrity("invalid registration version"));
        }
        let bytes = crate::utils::unhex(&marker.mac)
            .ok_or_else(|| integrity("invalid registration MAC"))?;
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(&self.authentication)
            .map_err(|_| Error::new(ErrorKind::Internal, "invalid MAC key"))?;
        mac.update(&claim_bytes(&marker.claims));
        mac.verify_slice(&bytes)
            .map_err(|_| integrity("registration authentication failed"))
    }
}

fn validate_identity(id: &DocumentIdentity) -> Result<()> {
    if id.schema_version != SCHEMA_VERSION {
        return Err(Error::new(
            ErrorKind::Compatibility,
            "unsupported encrypted index version",
        ));
    }
    if id.store_id.is_empty()
        || id.document_id.is_empty()
        || id.kind.is_empty()
        || id.key_generation == 0
    {
        return Err(integrity("invalid encrypted index identity"));
    }
    Ok(())
}

fn identity_bytes(id: &DocumentIdentity) -> Vec<u8> {
    frame(
        b"saucepan/encrypted-document/v1",
        &[
            &id.schema_version.to_be_bytes(),
            id.store_id.as_bytes(),
            id.kind.as_bytes(),
            id.document_id.as_bytes(),
            &id.key_generation.to_be_bytes(),
        ],
    )
}
fn claim_bytes(c: &MarkerClaims) -> Vec<u8> {
    frame(
        b"saucepan/registration/v1",
        &[
            &c.schema_version.to_be_bytes(),
            c.store_id.as_bytes(),
            c.registration_id.as_bytes(),
            &c.registration_revision.to_be_bytes(),
            c.enrolled_root.as_bytes(),
            c.nonce.as_bytes(),
            &c.key_generation.to_be_bytes(),
        ],
    )
}
fn integrity(message: &str) -> Error {
    Error::new(ErrorKind::Integrity, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> DocumentIdentity {
        DocumentIdentity {
            schema_version: 1,
            store_id: "test-store".into(),
            kind: "source".into(),
            document_id: "source-a".into(),
            key_generation: 1,
        }
    }
    fn claims() -> MarkerClaims {
        MarkerClaims {
            schema_version: 1,
            store_id: "test-store".into(),
            registration_id: "app".into(),
            registration_revision: 1,
            enrolled_root: "/app".into(),
            nonce: "fixed-test-nonce".into(),
            key_generation: 1,
        }
    }
    #[test]
    fn ciphertext_and_bound_identity_tampering_fail_before_plaintext() {
        let keys = Keys::derive(&[7; 32], "test-store").unwrap();
        let id = identity();
        let bytes = keys.seal(&id, br#"{"grants":["inspect"]}"#).unwrap();
        assert_eq!(
            &**keys.open(&id, &bytes).unwrap(),
            br#"{"grants":["inspect"]}"#
        );
        for field in ["store", "source", "kind", "generation"] {
            let mut changed = id.clone();
            match field {
                "store" => changed.store_id = "other".into(),
                "source" => changed.document_id = "other".into(),
                "kind" => changed.kind = "other".into(),
                _ => changed.key_generation += 1,
            }
            assert_eq!(
                keys.open(&changed, &bytes).unwrap_err().kind,
                ErrorKind::Integrity
            );
        }
        let mut envelope: EncryptedEnvelope = serde_json::from_slice(&bytes).unwrap();
        envelope.ciphertext[0] ^= 1;
        assert!(
            keys.open(&id, &serde_json::to_vec(&envelope).unwrap())
                .is_err()
        );
        envelope.nonce.truncate(3);
        assert!(
            keys.open(&id, &serde_json::to_vec(&envelope).unwrap())
                .is_err()
        );
    }
    #[test]
    fn markers_bind_stable_registration_not_lru_state() {
        let keys = Keys::derive(&[7; 32], "test-store").unwrap();
        let marker = keys.sign(claims()).unwrap();
        keys.verify(&marker).unwrap();
        let _unrelated_index_write = keys.seal(&identity(), b"new recency").unwrap();
        keys.verify(&marker).unwrap();
        for field in ["root", "revision", "store", "id", "nonce"] {
            let mut changed = keys.sign(claims()).unwrap();
            match field {
                "root" => changed.claims.enrolled_root = "/other".into(),
                "revision" => changed.claims.registration_revision += 1,
                "store" => changed.claims.store_id = "other".into(),
                "id" => changed.claims.registration_id = "other".into(),
                _ => changed.claims.nonce = "other".into(),
            }
            assert!(keys.verify(&changed).is_err());
        }
        let other = Keys::derive(&[7; 32], "other-store").unwrap();
        assert!(other.verify(&marker).is_err());
        assert_ne!(keys.encryption, keys.authentication);
    }
    #[test]
    fn hkdf_rfc5869_case_one() {
        let hkdf = Hkdf::<Sha256>::new(
            Some(&crate::utils::unhex("000102030405060708090a0b0c").unwrap()),
            &[0x0b; 22],
        );
        let mut out = [0; 42];
        hkdf.expand(
            &crate::utils::unhex("f0f1f2f3f4f5f6f7f8f9").unwrap(),
            &mut out,
        )
        .unwrap();
        assert_eq!(
            crate::utils::hex(&out),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
    }
    #[test]
    fn hmac_rfc4231_case_one() {
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(&[0x0b; 20]).unwrap();
        mac.update(b"Hi There");
        assert_eq!(
            crate::utils::hex(&mac.finalize().into_bytes()),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }
    #[test]
    fn repeated_seals_use_distinct_nonces_and_reject_oversize() {
        let keys = Keys::derive(&[7; 32], "test-store").unwrap();
        let id = identity();
        assert_ne!(
            keys.seal(&id, b"same").unwrap(),
            keys.seal(&id, b"same").unwrap()
        );
        assert!(keys.seal(&id, &vec![0; MAX_DOCUMENT_BYTES + 1]).is_err());
        assert!(keys.open(&id, b"not JSON").is_err());
        let mut future = id.clone();
        future.schema_version = 99;
        assert_eq!(
            keys.open(&future, b"{}").unwrap_err().kind,
            ErrorKind::Compatibility
        );
    }

    #[test]
    fn xchacha_draft_appendix_a1_vector() {
        // draft-irtf-cfrg-xchacha, Appendix A.1; independently specified bytes.
        let key: [u8; 32] = std::array::from_fn(|i| 0x80 + i as u8);
        let nonce: [u8; 24] = std::array::from_fn(|i| 0x40 + i as u8);
        let aad = crate::utils::unhex("50515253c0c1c2c3c4c5c6c7").unwrap();
        let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let cipher = XChaCha20Poly1305::new((&key).into());
        let encrypted = cipher
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .unwrap();
        assert_eq!(
            crate::utils::hex(&encrypted),
            concat!(
                "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb",
                "731c7f1b0b4aa6440bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b452",
                "2f8c9ba40db5d945b11b69b982c1bb9e3f3fac2bc369488f76b2383565d3fff9",
                "21f9664c97637da9768812f615c68b13b52ec0875924c1c7987947deafd8780acf49"
            )
        );
    }
}
