// The Unix side of the local provider's write primitives: the calls `std` does not offer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::write::FileTimes;
use crate::CancelToken;

/// What a fast copy did.
pub(crate) enum Fast {
    /// Nothing was touched; use the generic loop.
    Unhandled,
    Done(u64),
    Failed(io::Error),
    Cancelled,
}

fn c_path(path: &Path) -> io::Result<CString> {
    CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a path cannot contain NUL"))
}

fn check(status: libc::c_int) -> io::Result<()> {
    if status == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// `st_dev` of the entry itself (a symlink is not followed).
pub(crate) fn volume_id(path: &Path) -> io::Result<u64> {
    Ok(fs::symlink_metadata(path)?.dev())
}

// `time_t` and `tv_nsec` are narrower on some Unix targets, so the casts are needed there.
#[allow(clippy::unnecessary_cast)]
fn timespec(time: Option<SystemTime>) -> libc::timespec {
    match time {
        None => libc::timespec {
            tv_sec: 0,
            tv_nsec: libc::UTIME_OMIT,
        },
        Some(time) => {
            let (secs, nanos) = match time.duration_since(UNIX_EPOCH) {
                Ok(after) => (after.as_secs() as i64, after.subsec_nanos() as i64),
                Err(before) => {
                    let before = before.duration();
                    match before.subsec_nanos() {
                        0 => (-(before.as_secs() as i64), 0),
                        n => (-(before.as_secs() as i64) - 1, 1_000_000_000 - n as i64),
                    }
                }
            };
            libc::timespec {
                tv_sec: secs as libc::time_t,
                tv_nsec: nanos as _,
            }
        }
    }
}

/// Sets times on the entry itself (`AT_SYMLINK_NOFOLLOW`).
pub(crate) fn set_times(path: &Path, times: FileTimes) -> io::Result<()> {
    let c = c_path(path)?;
    let spec = [timespec(times.accessed), timespec(times.modified)];
    // SAFETY: `c` is NUL-terminated and `spec` holds the two timespecs the call reads.
    check(unsafe {
        libc::utimensat(
            libc::AT_FDCWD,
            c.as_ptr(),
            spec.as_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    })
}

/// Renames, failing with `EEXIST` when `to` exists if `overwrite` is false. On Linux that is one
/// atomic `renameat2(RENAME_NOREPLACE)`; where the kernel or file system lacks it (and on other
/// Unix systems) it checks first, which leaves a window for another process to create `to`.
pub(crate) fn rename(from: &Path, to: &Path, overwrite: bool) -> io::Result<()> {
    if overwrite {
        return fs::rename(from, to);
    }
    #[cfg(target_os = "linux")]
    {
        let (a, b) = (c_path(from)?, c_path(to)?);
        // SAFETY: both are NUL-terminated paths that outlive the call.
        let status = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                libc::AT_FDCWD,
                a.as_ptr(),
                libc::AT_FDCWD,
                b.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if status == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if !matches!(
            error.raw_os_error(),
            Some(libc::ENOSYS | libc::EINVAL | libc::EOPNOTSUPP)
        ) {
            return Err(error);
        }
    }
    match fs::symlink_metadata(to) {
        Ok(_) => Err(io::Error::from_raw_os_error(libc::EEXIST)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => fs::rename(from, to),
        Err(e) => Err(e),
    }
}

/// Opens `path` for writing, creating it. `mode` is the permission bits of a file this creates.
pub(crate) fn open_write(path: &Path, exclusive: bool, mode: Option<u32>) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true);
    if exclusive {
        options.create_new(true);
    } else {
        options.create(true).truncate(true);
    }
    if let Some(mode) = mode {
        options.mode(mode);
    }
    options.open(path)
}

#[cfg(target_os = "linux")]
mod linux {
    use std::os::fd::AsRawFd;

    use super::*;

    /// `FICLONE`: share the source's extents with the destination (a reflink).
    const FICLONE: u64 = 0x4004_9409;
    const CHUNK: usize = 8 * 1024 * 1024;

    /// Copies `src` to a new `dst` by reflink, or `copy_file_range` in chunks. Anything it cannot do
    /// before the first byte moves (another file system, an old kernel, a seccomp filter) leaves
    /// nothing behind and is `Unhandled`.
    pub(crate) fn copy(
        src: &Path,
        dst: &Path,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Fast {
        // A symlink source is `Unhandled` (as on Windows), so the caller decides what copying a link
        // means instead of this path silently copying its target.
        let input = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(src)
        {
            Ok(file) => file,
            Err(e) if e.raw_os_error() == Some(libc::ELOOP) => return Fast::Unhandled,
            Err(e) => return Fast::Failed(e),
        };
        let meta = match input.metadata() {
            Ok(meta) => meta,
            Err(e) => return Fast::Failed(e),
        };
        if !meta.is_file() {
            return Fast::Unhandled;
        }
        let len = meta.len();
        let output = match open_write(dst, true, Some(meta.mode() & 0o777)) {
            Ok(file) => file,
            Err(e) => return Fast::Failed(e),
        };
        let abandon = |fast: Fast| {
            let _ = fs::remove_file(dst);
            fast
        };
        // SAFETY: both descriptors are open for the duration of the call.
        let cloned = unsafe { libc::ioctl(output.as_raw_fd(), FICLONE as _, input.as_raw_fd()) };
        if cloned == 0 {
            progress(len);
            return Fast::Done(len);
        }
        let mut copied = 0u64;
        loop {
            if cancel.is_cancelled() {
                return abandon(Fast::Cancelled);
            }
            // SAFETY: both descriptors are open; null offsets use and advance the file offsets.
            let n = unsafe {
                libc::copy_file_range(
                    input.as_raw_fd(),
                    std::ptr::null_mut(),
                    output.as_raw_fd(),
                    std::ptr::null_mut(),
                    CHUNK,
                    0,
                )
            };
            if n < 0 {
                let error = io::Error::last_os_error();
                let unavailable = matches!(
                    error.raw_os_error(),
                    Some(
                        libc::EXDEV | libc::ENOSYS | libc::EINVAL | libc::EOPNOTSUPP | libc::EPERM
                    )
                );
                return abandon(if copied == 0 && unavailable {
                    Fast::Unhandled
                } else {
                    Fast::Failed(error)
                });
            }
            if n == 0 {
                break;
            }
            copied += n as u64;
            progress(copied);
        }
        Fast::Done(copied)
    }
}

#[cfg(target_os = "linux")]
pub(crate) use linux::copy as copy_fast;

#[cfg(not(target_os = "linux"))]
pub(crate) fn copy_fast(
    _src: &Path,
    _dst: &Path,
    _progress: &mut dyn FnMut(u64),
    _cancel: &CancelToken,
) -> Fast {
    Fast::Unhandled
}

/// The name of user `uid`, or `None` when the system has none.
pub(crate) fn user_name(uid: u32) -> Option<String> {
    let mut buffer = vec![0u8; 4096];
    loop {
        let mut entry = std::mem::MaybeUninit::<libc::passwd>::zeroed();
        let mut found: *mut libc::passwd = std::ptr::null_mut();
        // SAFETY: `entry`, `buffer` and `found` are valid for the call; on success `found` points
        // at `entry`, whose strings live in `buffer`, and both outlive the copy below.
        let status = unsafe {
            libc::getpwuid_r(
                uid,
                entry.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut found,
            )
        };
        if status == libc::ERANGE && buffer.len() < 1 << 20 {
            buffer.resize(buffer.len() * 2, 0);
            continue;
        }
        if status != 0 || found.is_null() {
            return None;
        }
        // SAFETY: the lookup succeeded, so `pw_name` is a NUL-terminated string in `buffer`.
        let name = unsafe { std::ffi::CStr::from_ptr((*found).pw_name) };
        return Some(name.to_string_lossy().into_owned());
    }
}

/// The name of group `gid`, or `None` when the system has none.
pub(crate) fn group_name(gid: u32) -> Option<String> {
    let mut buffer = vec![0u8; 4096];
    loop {
        let mut entry = std::mem::MaybeUninit::<libc::group>::zeroed();
        let mut found: *mut libc::group = std::ptr::null_mut();
        // SAFETY: as in `user_name`.
        let status = unsafe {
            libc::getgrgid_r(
                gid,
                entry.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut found,
            )
        };
        if status == libc::ERANGE && buffer.len() < 1 << 20 {
            buffer.resize(buffer.len() * 2, 0);
            continue;
        }
        if status != 0 || found.is_null() {
            return None;
        }
        // SAFETY: the lookup succeeded, so `gr_name` is a NUL-terminated string in `buffer`.
        let name = unsafe { std::ffi::CStr::from_ptr((*found).gr_name) };
        return Some(name.to_string_lossy().into_owned());
    }
}

/// Lowers the calling thread's CPU priority (nice 19) and, on Linux, puts its disk I/O in the idle
/// class, so a long walk yields to anything the person is doing. Failures are ignored: the walk
/// is only slower to yield.
pub(crate) fn lower_thread_priority() {
    // On Linux `setpriority(PRIO_PROCESS, 0, …)` applies to the calling thread, not the process.
    // SAFETY: plain system calls with no pointers.
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, 0, 19);
        #[cfg(target_os = "linux")]
        {
            const IOPRIO_WHO_PROCESS: libc::c_long = 1;
            const IOPRIO_CLASS_IDLE: libc::c_long = 3;
            libc::syscall(
                libc::SYS_ioprio_set,
                IOPRIO_WHO_PROCESS,
                0 as libc::c_long,
                IOPRIO_CLASS_IDLE << 13,
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::unnecessary_cast)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn times_before_the_epoch_split_into_floor_seconds_and_positive_nanos() {
        let before = UNIX_EPOCH - Duration::from_millis(1500);
        let spec = timespec(Some(before));
        assert_eq!((spec.tv_sec as i64, spec.tv_nsec as i64), (-2, 500_000_000));
        let whole = timespec(Some(UNIX_EPOCH - Duration::from_secs(3)));
        assert_eq!((whole.tv_sec as i64, whole.tv_nsec as i64), (-3, 0));
        let after = timespec(Some(UNIX_EPOCH + Duration::new(5, 7)));
        assert_eq!((after.tv_sec as i64, after.tv_nsec as i64), (5, 7));
        assert_eq!(timespec(None).tv_nsec, libc::UTIME_OMIT);
    }

    #[test]
    fn a_pre_epoch_time_round_trips_on_a_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old");
        fs::write(&path, b"x").unwrap();
        let old = UNIX_EPOCH - Duration::from_millis(1500);
        let times = FileTimes {
            accessed: None,
            modified: Some(old),
        };
        // Some file systems refuse negative times; the conversion is what is under test.
        if set_times(&path, times).is_ok() {
            assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), old);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_fast_copy_leaves_a_symlink_source_to_the_caller() {
        let dir = tempfile::tempdir().unwrap();
        let (real, link, dst) = (
            dir.path().join("real"),
            dir.path().join("link"),
            dir.path().join("dst"),
        );
        fs::write(&real, b"data").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let fast = copy_fast(&link, &dst, &mut |_| {}, &CancelToken::new());
        assert!(matches!(fast, Fast::Unhandled));
        assert!(!dst.exists());
        let fast = copy_fast(&real, &dst, &mut |_| {}, &CancelToken::new());
        assert!(matches!(fast, Fast::Done(4)));
    }

    #[test]
    fn rename_without_overwrite_refuses_an_existing_target_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a"), dir.path().join("b"));
        fs::write(&a, b"a").unwrap();
        fs::write(&b, b"b").unwrap();
        let error = rename(&a, &b, false).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(libc::EEXIST));
        assert_eq!(fs::read(&b).unwrap(), b"b");
        rename(&a, &b, true).unwrap();
        assert_eq!(fs::read(&b).unwrap(), b"a");
    }
}
