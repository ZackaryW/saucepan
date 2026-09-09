//! Byte-oriented cryptographic primitives. Callers own key custody, domain labels,
//! payload serialization, and format versions; no application policy lives here.
use chacha20poly1305::{
    XChaCha20Poly1305,
    aead::{Aead, KeyInit, Payload},
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::io::{self, Read};
use zeroize::Zeroizing;

/// Fill a fixed-size buffer from the operating system's random source.
pub fn random_bytes<const N: usize>() -> io::Result<Zeroizing<[u8; N]>> {
    let mut bytes = Zeroizing::new([0; N]);
    getrandom::fill(bytes.as_mut()).map_err(io::Error::other)?;
    Ok(bytes)
}

/// HKDF-SHA256. Use distinct `info` labels for independent key purposes.
pub fn derive_key<const N: usize>(
    secret: &[u8],
    salt: Option<&[u8]>,
    info: &[u8],
) -> io::Result<Zeroizing<[u8; N]>> {
    let mut output = Zeroizing::new([0; N]);
    hkdf::Hkdf::<Sha256>::new(salt, secret)
        .expand(info, output.as_mut())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "HKDF output is too long"))?;
    Ok(output)
}

/// HMAC-SHA256 over an arbitrary byte stream.
pub fn authenticate(key: &[u8], reader: impl Read) -> io::Result<[u8; 32]> {
    Ok(mac(key, reader)?.finalize().into_bytes().into())
}

/// Verify with the cryptographic library's constant-time tag comparison.
pub fn verify(key: &[u8], reader: impl Read, tag: &[u8]) -> io::Result<()> {
    mac(key, reader)?
        .verify_slice(tag)
        .map_err(|_| authentication_error())
}

fn mac(key: &[u8], mut reader: impl Read) -> io::Result<Hmac<Sha256>> {
    struct Writer(Hmac<Sha256>);
    impl io::Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Writer(
        <Hmac<Sha256> as hmac::KeyInit>::new_from_slice(key)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid HMAC key"))?,
    );
    io::copy(&mut reader, &mut writer)?;
    Ok(writer.0)
}

fn authentication_error() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "authentication failed")
}

/// XChaCha20-Poly1305 output, without an application-specific storage envelope.
pub struct Sealed {
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
}

/// Encrypt with a newly generated nonce; `context` is authenticated but not encrypted.
pub fn seal(key: &[u8; 32], plaintext: &[u8], context: &[u8]) -> io::Result<Sealed> {
    let nonce = *random_bytes::<24>()?;
    let ciphertext = XChaCha20Poly1305::new(key.into())
        .encrypt(
            (&nonce).into(),
            Payload {
                msg: plaintext,
                aad: context,
            },
        )
        .map_err(|_| io::Error::other("encryption failed"))?;
    Ok(Sealed { nonce, ciphertext })
}

/// Authenticate before returning plaintext. Wrong keys/context and corrupt data fail.
pub fn open(key: &[u8; 32], sealed: &Sealed, context: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
    XChaCha20Poly1305::new(key.into())
        .decrypt(
            (&sealed.nonce).into(),
            Payload {
                msg: &sealed.ciphertext,
                aad: context,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| authentication_error())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn hkdf_matches_rfc5869_case_one_and_separates_contexts() {
        let salt: Vec<u8> = (0..=12).collect();
        let info: Vec<u8> = (240..=249).collect();
        let key = derive_key::<42>(&[0x0b; 22], Some(&salt), &info).unwrap();
        assert_eq!(
            hex(key.as_ref()),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
        assert_ne!(
            *derive_key::<32>(b"secret", None, b"one").unwrap(),
            *derive_key::<32>(b"secret", None, b"two").unwrap()
        );
        assert!(derive_key::<8161>(b"secret", None, b"too long").is_err());
    }

    #[test]
    fn hmac_matches_rfc4231_and_rejects_changed_data_keys_and_tag_lengths() {
        let key = [0x0b; 20];
        let tag = authenticate(&key, b"Hi There".as_slice()).unwrap();
        assert_eq!(
            hex(&tag),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        verify(&key, b"Hi There".as_slice(), &tag).unwrap();
        assert!(verify(&key, b"Hi there".as_slice(), &tag).is_err());
        assert!(verify(b"wrong", b"Hi There".as_slice(), &tag).is_err());
        assert!(verify(&key, b"Hi There".as_slice(), &tag[..31]).is_err());
        assert!(verify(&key, b"Hi There".as_slice(), &[]).is_err());
    }

    #[test]
    fn stream_errors_cannot_be_authenticated_as_partial_data() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("broken stream"))
            }
        }
        assert_eq!(
            authenticate(b"key", Broken).unwrap_err().to_string(),
            "broken stream"
        );
        assert_eq!(
            verify(b"key", Broken, &[0; 32]).unwrap_err().to_string(),
            "broken stream"
        );
    }

    #[test]
    fn random_buffers_and_encryption_use_fresh_randomness() {
        assert_ne!(
            *random_bytes::<32>().unwrap(),
            *random_bytes::<32>().unwrap()
        );
        let first = seal(&[3; 32], b"payload", b"context").unwrap();
        let second = seal(&[3; 32], b"payload", b"context").unwrap();
        assert_ne!(first.nonce, second.nonce);
        assert_ne!(first.ciphertext, second.ciphertext);
        assert_eq!(&**open(&[3; 32], &first, b"context").unwrap(), b"payload");
        assert_eq!(
            &**open(&[3; 32], &seal(&[3; 32], b"", b"").unwrap(), b"").unwrap(),
            b""
        );
    }

    #[test]
    fn decryption_rejects_wrong_key_context_tampering_and_truncation() {
        let mut sealed = seal(&[3; 32], b"payload", b"context").unwrap();
        assert!(open(&[4; 32], &sealed, b"context").is_err());
        assert!(open(&[3; 32], &sealed, b"other").is_err());
        sealed.nonce[0] ^= 1;
        assert!(open(&[3; 32], &sealed, b"context").is_err());
        sealed.nonce[0] ^= 1;
        sealed.ciphertext[0] ^= 1;
        assert!(open(&[3; 32], &sealed, b"context").is_err());
        sealed.ciphertext.clear();
        assert!(open(&[3; 32], &sealed, b"context").is_err());
    }
}
