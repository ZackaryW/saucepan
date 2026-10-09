use crate::{
    error::{Error, ErrorCode},
    models::claim::security::Credential,
    utils::path::plain_metadata,
};
use anyhow::{Result, ensure};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

pub(super) fn read(path: Option<&Path>) -> Result<Option<Credential>> {
    path.map(|path| {
        ensure!(
            plain_metadata(path)?.is_file(),
            Error::new(
                ErrorCode::InvalidRequest,
                "credential must be a regular file"
            )
        );
        let mut bytes = zeroize::Zeroizing::new(Vec::new());
        File::open(path)?.take(33).read_to_end(&mut bytes)?;
        Ok(Credential::from_bytes(&bytes)?)
    })
    .transpose()
}

pub(super) fn deliver(path: Option<&Path>, token: &Credential) -> Result<()> {
    let path = path.ok_or_else(|| {
        Error::new(
            ErrorCode::InvalidRequest,
            "--credential-out is required for a new credential",
        )
    })?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0).access_mode(0x40000000 | 0x00040000); // GENERIC_WRITE | WRITE_DAC
    }
    let mut file = options.open(path)?;
    #[cfg(windows)]
    restrict(&file)?;
    file.write_all(token.expose())?;
    file.sync_all()?;
    Ok(())
}

#[cfg(windows)]
fn restrict(file: &File) -> Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows::{
        Win32::{
            Foundation::{HANDLE, HLOCAL, LocalFree},
            Security::{
                Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW,
                DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
                PSECURITY_DESCRIPTOR, SetKernelObjectSecurity,
            },
        },
        core::w,
    };
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: the API allocates a self-relative descriptor; its storage remains
    // valid until LocalFree, including the synchronous handle-based ACL update.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            w!("D:P(A;;FA;;;OW)"),
            1,
            &mut descriptor,
            None,
        )?;
        let result = SetKernelObjectSecurity(
            HANDLE(file.as_raw_handle()),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        );
        let _ = LocalFree(Some(HLOCAL(descriptor.0)));
        result?;
    }
    Ok(())
}
