use saucepan::utils::{fs::staged_directory, transfer::copy_bounded};
use std::io::{self, Read, Write};

struct Short<T>(T);
impl<T: Read> Read for Short<T> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.0.read(&mut buffer[..1])
    }
}
impl<T: Write> Write for Short<T> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0.write(&buffer[..1])
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[test]
fn boundaries_short_io_and_optional_hash() {
    let mut output = Vec::new();
    let result = copy_bounded(Short(&b"abc"[..]), Short(&mut output), 3, true).unwrap();
    assert_eq!(output, b"abc");
    assert_eq!(result.bytes, 3);
    assert_eq!(
        result.sha256.unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert!(
        copy_bounded(&b""[..], io::sink(), 0, false)
            .unwrap()
            .sha256
            .is_none()
    );
    assert!(copy_bounded(&b"x"[..], io::sink(), 0, true).is_err());
    let mut output = Vec::new();
    assert!(copy_bounded(Short(&b"abcd"[..]), &mut output, 3, false).is_err());
    assert_eq!(output, b"abc");
    assert_eq!(
        copy_bounded(&b"abc"[..], io::sink(), u64::MAX, false)
            .unwrap()
            .bytes,
        3
    );
}

struct Broken;
impl Read for Broken {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("reader failed"))
    }
}
impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("writer failed"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn failed_transfer_never_publishes_staged_destination() {
    assert!(copy_bounded(Broken, io::sink(), 8, false).is_err());
    assert!(copy_bounded(&b"abc"[..], Broken, 8, false).is_err());
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("result");
    assert!(
        staged_directory(&destination, |stage| {
            let file = std::fs::File::create(stage.join("file"))?;
            copy_bounded(&b"too long"[..], file, 3, true)?;
            Ok(())
        })
        .is_err()
    );
    assert!(!destination.exists());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
