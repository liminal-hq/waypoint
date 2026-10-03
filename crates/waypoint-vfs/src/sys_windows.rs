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
    CopyFileExW, GetFileInformationByHandle, MoveFileExW, BY_HANDLE_FILE_INFORMATION,
    COPYFILE_FLAGS, COPYPROGRESSROUTINE_PROGRESS, COPY_FILE_FAIL_IF_EXISTS,
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
pub(crate) fn rename(from: &Path, to: &Path, overwrite: bool) -> io::Result<()> {
    // `MoveFileExW` treats a rename onto the very same path as a successful no-op, but a target
    // that is there is `AlreadyExists` without `overwrite`, itself included. (A case-only rename
    // names a different path and goes through.)
    if !overwrite && from == to && fs::symlink_metadata(to).is_ok() {
        return Err(io::Error::from_raw_os_error(ERROR_ALREADY_EXISTS));
    }
    let flags = if overwrite {
        MOVEFILE_REPLACE_EXISTING
    } else {
        MOVE_FILE_FLAGS(0)
    };
    let (a, b) = (wide(from), wide(to));
    // SAFETY: both are NUL-terminated and outlive the call.
    unsafe { MoveFileExW(PCWSTR(a.as_ptr()), PCWSTR(b.as_ptr()), flags) }.map_err(win_error)
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
