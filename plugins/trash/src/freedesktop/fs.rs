// The two file system calls of the trash that tests need to fake: the device a path is on, and renaming
//
// A real rename across devices cannot be provoked in a temporary directory, and neither can two devices, so the trash logic asks this trait. `StdFs` is the real thing.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// The file system operations of the trash that depend on the device a path is on.
pub trait TrashFs: Send + Sync {
    /// The device number (`st_dev`) of the file itself; a link is not followed.
    fn device_id(&self, path: &Path) -> io::Result<u64>;

    /// The owner (`st_uid`) and the permission bits (`st_mode`) of the file itself; a link is not followed. The trash refuses a directory that someone else owns or others can write, so tests fake it.
    fn owner_and_mode(&self, path: &Path) -> io::Result<(u32, u32)> {
        let metadata = std::fs::symlink_metadata(path)?;
        Ok((metadata.uid(), metadata.mode()))
    }

    /// Renames, replacing a file that is already at `to`.
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;

    /// Renames, failing with `AlreadyExists` instead of replacing anything at `to`.
    fn rename_noreplace(&self, from: &Path, to: &Path) -> io::Result<()>;
}

/// The real file system.
#[derive(Debug, Default, Clone, Copy)]
pub struct StdFs;

impl TrashFs for StdFs {
    fn device_id(&self, path: &Path) -> io::Result<u64> {
        Ok(std::fs::symlink_metadata(path)?.dev())
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    fn rename_noreplace(&self, from: &Path, to: &Path) -> io::Result<()> {
        let c_from = CString::new(from.as_os_str().as_bytes())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        let c_to = CString::new(to.as_os_str().as_bytes())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        // SAFETY: both pointers are valid NUL-terminated strings that outlive the call.
        let result = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                libc::AT_FDCWD,
                c_from.as_ptr(),
                libc::AT_FDCWD,
                c_to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            // The kernel or the file system has no `RENAME_NOREPLACE`: check, then rename. Another process can still slip in between, but only on file systems that old.
            Some(libc::ENOSYS) | Some(libc::EINVAL) | Some(libc::EOPNOTSUPP) => {
                if std::fs::symlink_metadata(to).is_ok() {
                    return Err(io::Error::from(io::ErrorKind::AlreadyExists));
                }
                std::fs::rename(from, to)
            }
            // `renameat2` reports EEXIST; `ErrorKind::AlreadyExists` is what the callers match.
            _ => Err(error),
        }
    }
}
