// The Windows side of the local provider's write primitives: the calls `std` does not offer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::c_void;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use windows::core::PCWSTR;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    CopyFileExW, FileRenameInfoEx, GetFileInformationByHandle, MoveFileExW,
    SetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, COPYFILE_FLAGS,
    COPYPROGRESSROUTINE_PROGRESS, COPY_FILE_FAIL_IF_EXISTS, FILE_RENAME_INFO,
    LPPROGRESS_ROUTINE_CALLBACK_REASON, MOVEFILE_REPLACE_EXISTING, MOVE_FILE_FLAGS,
    PROGRESS_CANCEL, PROGRESS_CONTINUE,
};

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

const FILE_READ_ATTRIBUTES: u32 = 0x80;
const FILE_WRITE_ATTRIBUTES: u32 = 0x100;
const FILE_SHARE_ALL: u32 = 0x7;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const ERROR_REQUEST_ABORTED: i32 = 1235;
const ERROR_ALREADY_EXISTS: i32 = 183;
const ERROR_ACCESS_DENIED: i32 = 5;
const ERROR_SHARING_VIOLATION: i32 = 32;
const ERROR_INVALID_FUNCTION: i32 = 1;
const ERROR_INVALID_PARAMETER: i32 = 87;
const ERROR_NOT_SUPPORTED: i32 = 50;
const ERROR_CALL_NOT_IMPLEMENTED: i32 = 120;
const DELETE: u32 = 0x1_0000;
const FILE_RENAME_FLAG_REPLACE_IF_EXISTS: u32 = 0x1;
const FILE_RENAME_FLAG_POSIX_SEMANTICS: u32 = 0x2;

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// A Win32 error as an `io::Error`, keeping the code so `from_io` can classify it.
fn win_error(error: windows::core::Error) -> io::Error {
    let code = error.code().0 as u32;
    if code >> 16 == 0x8007 {
        io::Error::from_raw_os_error((code & 0xFFFF) as i32)
    } else {
        io::Error::other(error)
    }
}

/// Opens the entry itself (a folder too, a symlink not followed) with the given access.
fn open_entry(path: &Path, access: u32) -> io::Result<File> {
    OpenOptions::new()
        .access_mode(access)
        .share_mode(FILE_SHARE_ALL)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

/// The volume serial number, from `GetFileInformationByHandleW` on a handle to the entry.
pub(crate) fn volume_id(path: &Path) -> io::Result<u64> {
    let file = open_entry(path, FILE_READ_ATTRIBUTES)?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: the handle is open for the duration of the call and `info` is writable.
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info) }
        .map_err(win_error)?;
    Ok(u64::from(info.dwVolumeSerialNumber))
}

/// Sets times on the entry itself (a reparse point is opened, not followed).
pub(crate) fn set_times(path: &Path, times: FileTimes) -> io::Result<()> {
    let file = open_entry(path, FILE_WRITE_ATTRIBUTES)?;
    let mut set = fs::FileTimes::new();
    if let Some(accessed) = times.accessed {
        set = set.set_accessed(accessed);
    }
    if let Some(modified) = times.modified {
        set = set.set_modified(modified);
    }
    file.set_times(set)
}

/// Renames within a volume. Without `MOVEFILE_REPLACE_EXISTING` an existing target is
/// `ERROR_ALREADY_EXISTS`, atomically; without `MOVEFILE_COPY_ALLOWED` another volume is
/// `ERROR_NOT_SAME_DEVICE`.
///
/// A replacing rename uses POSIX semantics (`FileRenameInfoEx`, Windows 10 1809 and later): the
/// target's name is unlinked at once and a reader that still has it open (an open listing, a
/// verify read) keeps its handle, where `MoveFileExW(MOVEFILE_REPLACE_EXISTING)` is refused with
/// `ERROR_ACCESS_DENIED` while any handle to the target is open. A system without it falls back to
/// `MoveFileExW`, and a replace that is still refused after a few short waits is reported as a
/// sharing violation (`InUse`) when the target is a writable file.
pub(crate) fn rename(from: &Path, to: &Path, overwrite: bool) -> io::Result<()> {
    // `MoveFileExW` treats a rename onto the very same path as a successful no-op, but a target
    // that is there is `AlreadyExists` without `overwrite`, itself included. (A case-only rename
    // names a different path and goes through.)
    if !overwrite && from == to && fs::symlink_metadata(to).is_ok() {
        return Err(io::Error::from_raw_os_error(ERROR_ALREADY_EXISTS));
    }
    if !overwrite || from == to {
        return move_file(from, to, overwrite);
    }
    // Only a file target is worth waiting for: a folder that would be replaced is refused for good.
    let patient = fs::symlink_metadata(to).is_ok_and(|meta| meta.is_file());
    let mut waited = 0;
    loop {
        match posix_replace(from, to) {
            Ok(()) => return Ok(()),
            Err(error) if is_unsupported(&error) => match move_file(from, to, true) {
                Err(error) if patient && is_busy(&error) && waited < RETRY_WAITS.len() => {}
                other => return other.map_err(|error| busy_if_held(error, to)),
            },
            Err(error) if patient && is_busy(&error) && waited < RETRY_WAITS.len() => {}
            Err(error) => return Err(busy_if_held(error, to)),
        }
        std::thread::sleep(std::time::Duration::from_millis(RETRY_WAITS[waited]));
        waited += 1;
    }
}

/// How long to wait before each new try of a refused replace.
const RETRY_WAITS: [u64; 5] = [10, 25, 50, 100, 200];

fn is_busy(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION)
    )
}

fn is_unsupported(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(
            ERROR_INVALID_FUNCTION
                | ERROR_INVALID_PARAMETER
                | ERROR_NOT_SUPPORTED
                | ERROR_CALL_NOT_IMPLEMENTED
        )
    )
}

/// A refusal of a replace that comes from an open handle is a sharing violation, which callers
/// word as "in use"; a read-only or missing target keeps the error it had.
fn busy_if_held(error: io::Error, target: &Path) -> io::Error {
    if error.raw_os_error() == Some(ERROR_ACCESS_DENIED)
        && fs::metadata(target).is_ok_and(|meta| meta.is_file() && !meta.permissions().readonly())
    {
        return io::Error::from_raw_os_error(ERROR_SHARING_VIOLATION);
    }
    error
}

fn move_file(from: &Path, to: &Path, overwrite: bool) -> io::Result<()> {
    let flags = if overwrite {
        MOVEFILE_REPLACE_EXISTING
    } else {
        MOVE_FILE_FLAGS(0)
    };
    let (a, b) = (wide(from), wide(to));
    // SAFETY: both are NUL-terminated and outlive the call.
    unsafe { MoveFileExW(PCWSTR(a.as_ptr()), PCWSTR(b.as_ptr()), flags) }.map_err(win_error)
}

/// Renames `from` over `to` through a handle to `from`, replacing an existing target with POSIX
/// semantics.
fn posix_replace(from: &Path, to: &Path) -> io::Result<()> {
    // The new name as a full path, in the `\\?\` form the call takes.
    let full = std::path::absolute(to)?;
    let mut name: Vec<u16> = full.as_os_str().encode_wide().collect();
    let verbatim: Vec<u16> = r"\\?\".encode_utf16().collect();
    if !name.starts_with(&verbatim) {
        let unc: Vec<u16> = r"\\".encode_utf16().collect();
        if name.starts_with(&unc) {
            let mut with: Vec<u16> = r"\\?\UNC\".encode_utf16().collect();
            with.extend_from_slice(&name[2..]);
            name = with;
        } else {
            let mut with = verbatim;
            with.extend_from_slice(&name);
            name = with;
        }
    }
    let source = open_entry(from, DELETE)?;
    let header = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let bytes = header + name.len() * 2;
    // A buffer aligned for the struct, which ends in the name.
    let mut buffer = vec![0u64; bytes.div_ceil(8) + 1];
    let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    // SAFETY: the buffer is zeroed, aligned and large enough for the header and the name, which is
    // copied in after it; the handle is open for the call.
    unsafe {
        (*info).Anonymous.Flags =
            FILE_RENAME_FLAG_REPLACE_IF_EXISTS | FILE_RENAME_FLAG_POSIX_SEMANTICS;
        (*info).FileNameLength = (name.len() * 2) as u32;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            std::ptr::addr_of_mut!((*info).FileName).cast::<u16>(),
            name.len(),
        );
        SetFileInformationByHandle(
            HANDLE(source.as_raw_handle()),
            FileRenameInfoEx,
            info.cast::<c_void>(),
            bytes as u32,
        )
    }
    .map_err(win_error)
}

/// Opens `path` for writing, creating it.
pub(crate) fn open_write(path: &Path, exclusive: bool, _mode: Option<u32>) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true);
    if exclusive {
        options.create_new(true);
    } else {
        options.create(true).truncate(true);
    }
    options.open(path).map_err(|error| {
        // `CREATE_NEW` over an existing folder is `ERROR_ACCESS_DENIED`, not `ERROR_FILE_EXISTS`.
        if exclusive
            && error.kind() == io::ErrorKind::PermissionDenied
            && fs::symlink_metadata(path).is_ok()
        {
            io::Error::from_raw_os_error(ERROR_ALREADY_EXISTS)
        } else {
            error
        }
    })
}

struct CopyState<'a> {
    progress: &'a mut dyn FnMut(u64),
    cancel: &'a CancelToken,
}

unsafe extern "system" fn routine(
    _total: i64,
    transferred: i64,
    _stream_size: i64,
    _stream_transferred: i64,
    _stream: u32,
    _reason: LPPROGRESS_ROUTINE_CALLBACK_REASON,
    _source: HANDLE,
    _destination: HANDLE,
    data: *const c_void,
) -> COPYPROGRESSROUTINE_PROGRESS {
    // SAFETY: `data` is the `CopyState` `copy` passed, alive for the whole `CopyFileExW` call.
    let state = unsafe { &mut *(data as *mut CopyState) };
    (state.progress)(transferred.max(0) as u64);
    if state.cancel.is_cancelled() {
        PROGRESS_CANCEL
    } else {
        PROGRESS_CONTINUE
    }
}

/// Copies a regular file with `CopyFileExW`, failing if `dst` exists. Windows removes the partial
/// destination when the callback cancels.
pub(crate) fn copy_fast(
    src: &Path,
    dst: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &CancelToken,
) -> Fast {
    match fs::symlink_metadata(src) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return Fast::Unhandled,
        Err(e) => return Fast::Failed(e),
    }
    let len = fs::metadata(src).map_or(0, |m| m.len());
    let (a, b) = (wide(src), wide(dst));
    let mut state = CopyState { progress, cancel };
    // SAFETY: the paths are NUL-terminated; `state` outlives the call and the routine only reads it
    // through the pointer; no cancel flag is passed.
    let result = unsafe {
        CopyFileExW(
            PCWSTR(a.as_ptr()),
            PCWSTR(b.as_ptr()),
            Some(routine),
            Some(&mut state as *mut CopyState as *const c_void),
            None,
            COPYFILE_FLAGS(COPY_FILE_FAIL_IF_EXISTS.0),
        )
    };
    match result {
        Ok(()) => Fast::Done(len),
        Err(e) => {
            let error = win_error(e);
            if cancel.is_cancelled() && error.raw_os_error() == Some(ERROR_REQUEST_ABORTED) {
                Fast::Cancelled
            } else {
                Fast::Failed(error)
            }
        }
    }
}

/// What the file takes on disk (its compressed or sparse size), from `GetCompressedFileSizeW`,
/// which reads the file's metadata and never its data.
pub(crate) fn allocated_size(path: &Path) -> Option<u64> {
    use windows::Win32::Foundation::{GetLastError, NO_ERROR};
    use windows::Win32::Storage::FileSystem::GetCompressedFileSizeW;

    let wide = wide(path);
    let mut high = 0u32;
    // SAFETY: `wide` is NUL-terminated and outlives the call; `high` is writable.
    let low = unsafe { GetCompressedFileSizeW(PCWSTR(wide.as_ptr()), Some(&mut high)) };
    // `INVALID_FILE_SIZE` is also a legitimate low half, so the error state decides.
    // SAFETY: reads the calling thread's last-error value.
    if low == u32::MAX && unsafe { GetLastError() } != NO_ERROR {
        return None;
    }
    Some((u64::from(high) << 32) | u64::from(low))
}

/// Puts the calling thread in background mode, which lowers its CPU, disk and memory priority so
/// a long walk yields to what the person is doing. A failure is ignored: the walk is only slower
/// to yield.
pub(crate) fn lower_thread_priority() {
    use windows::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
    };
    // SAFETY: the pseudo-handle of the current thread is always valid.
    let _ = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replacing_rename_goes_through_while_the_target_is_open_for_reading() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("new"), dir.path().join("old"));
        fs::write(&from, b"new").unwrap();
        fs::write(&to, b"old").unwrap();
        let held = File::open(&to).unwrap();
        rename(&from, &to, true).expect("POSIX semantics replace an open target");
        drop(held);
        assert_eq!(fs::read(&to).unwrap(), b"new");
        assert!(!from.exists());
    }

    #[test]
    fn a_replacing_rename_refused_for_good_is_a_sharing_violation() {
        let dir = tempfile::tempdir().unwrap();
        let (from, to) = (dir.path().join("new"), dir.path().join("old"));
        fs::write(&from, b"new").unwrap();
        fs::write(&to, b"old").unwrap();
        // Opened without delete sharing, as most programs do: nothing can replace it.
        let _held = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&to)
            .unwrap();
        let error = rename(&from, &to, true).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(ERROR_SHARING_VIOLATION));
        assert_eq!(fs::read(&to).unwrap(), b"old");
    }
}
