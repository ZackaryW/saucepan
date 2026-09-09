use std::io::{self, Read};

use sha2::{Digest, Sha256};

/// Hash a byte stream as lowercase SHA-256 hex without buffering the whole input.
pub fn sha256(mut reader: impl Read) -> io::Result<String> {
    let mut digest = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => digest.update(&buffer[..count]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    Ok(digest
        .finalize()
        .iter()
        .flat_map(|byte| [HEX[(byte >> 4) as usize], HEX[(byte & 15) as usize]])
        .map(char::from)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_standard_vectors() {
        for (input, expected) in [
            (
                "",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                "abc",
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
        ] {
            assert_eq!(sha256(input.as_bytes()).unwrap(), expected);
        }
    }

    #[test]
    fn hashes_a_stream_larger_than_a_buffer() {
        assert_eq!(
            sha256(io::repeat(b'a').take(1_000_000)).unwrap(),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn tolerates_interruptions_and_short_reads() {
        struct Chunks {
            interrupted: bool,
            bytes: &'static [u8],
        }
        impl Read for Chunks {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                if !self.interrupted {
                    self.interrupted = true;
                    return Err(io::ErrorKind::Interrupted.into());
                }
                let count = buffer.len().min(1);
                self.bytes.read(&mut buffer[..count])
            }
        }
        let reader = Chunks {
            interrupted: false,
            bytes: b"abc",
        };
        assert_eq!(
            sha256(reader).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn propagates_read_errors_instead_of_hashing_partial_input() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("read failed"))
            }
        }
        let reader = io::Cursor::new(b"partial").chain(Broken);
        let error = sha256(reader).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(error.to_string(), "read failed");
    }
}
