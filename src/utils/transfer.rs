//! Bounded byte transfer. The caller owns staging, publication and verification.
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};

#[derive(Debug, PartialEq, Eq)]
pub struct Transferred {
    pub bytes: u64,
    pub sha256: Option<String>,
}

/// Reads at most limit + 1 bytes to detect overflow, never writes over the limit.
/// Errors can leave a partial writer; callers must not publish it on failure.
pub fn copy_bounded(
    mut reader: impl Read,
    mut writer: impl Write,
    limit: u64,
    hash: bool,
) -> io::Result<Transferred> {
    let mut sha256 = hash.then(Sha256::new);
    let mut bytes = 0u64;
    let mut buffer = [0; 65_536];
    loop {
        let remaining = limit - bytes;
        let length = remaining.saturating_add(1).min(buffer.len() as u64) as usize;
        let count = match reader.read(&mut buffer[..length]) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            break;
        }
        if count as u64 > remaining {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "byte limit exceeded",
            ));
        }
        writer.write_all(&buffer[..count])?;
        if let Some(hash) = &mut sha256 {
            hash.update(&buffer[..count]);
        }
        bytes += count as u64;
    }
    Ok(Transferred {
        bytes,
        sha256: sha256.map(|hash| hex::encode(hash.finalize())),
    })
}
