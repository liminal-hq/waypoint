// Free and total space on the volume that holds a location
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_path::FilePath;
use waypoint_protocol::Location;

use crate::model::VolumeSpace;

/// The space on the volume holding `location`, or `None` where it cannot be determined (the
/// location is not local, does not exist, or the volume does not report it). Free space is what an
/// ordinary user may use, not what root could.
pub fn free_space(location: &Location) -> Option<VolumeSpace> {
    let path = FilePath::from_location(location).ok()?;
    query(&path)
}

// The `statvfs` fields are `u32` on some Unix targets and `u64` on others, so the widening casts
// are needed there even though they are no-ops on Linux x86_64.
#[cfg(unix)]
#[allow(clippy::useless_conversion, clippy::unnecessary_cast)]
pub(crate) fn query(path: &FilePath) -> Option<VolumeSpace> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_path().as_os_str().as_bytes()).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    // SAFETY: `c_path` is a valid NUL-terminated string and `stat` points at writable memory of
    // the right type; `statvfs` fully initialises it when it returns 0.
    let status = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if status != 0 {
        return None;
    }
    // SAFETY: the call succeeded, so the structure is initialised.
    let stat = unsafe { stat.assume_init() };
    let unit = u64::from(stat.f_frsize);
    let total = (stat.f_blocks as u64).saturating_mul(unit);
    if total == 0 {
        // Virtual file systems report no blocks; there is nothing meaningful to show.
        return None;
    }
    Some(VolumeSpace {
        free_bytes: (stat.f_bavail as u64).saturating_mul(unit),
        total_bytes: total,
    })
}

#[cfg(windows)]
pub(crate) fn query(path: &FilePath) -> Option<VolumeSpace> {
    use std::os::windows::ffi::OsStrExt;

    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide: Vec<u16> = path
        .as_path()
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let (mut available, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: `wide` is NUL-terminated and outlives the call; the out pointers are valid.
    unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(wide.as_ptr()),
            Some(&mut available),
            Some(&mut total),
            Some(&mut free),
        )
    }
    .ok()?;
    Some(VolumeSpace {
        free_bytes: available,
        total_bytes: total,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_real_folder_reports_space() {
        let dir = tempfile::tempdir().unwrap();
        let location = FilePath::from_path(dir.path()).unwrap().to_location();
        let space = free_space(&location).expect("a temp folder's volume reports space");
        assert!(space.total_bytes > 0);
        assert!(space.free_bytes <= space.total_bytes);
    }

    #[test]
    fn a_missing_or_malformed_location_reports_nothing() {
        let missing = FilePath::parse("/no/such/folder/anywhere")
            .unwrap()
            .to_location();
        assert_eq!(free_space(&missing), None);
        assert_eq!(free_space(&Location::new("x", "nonsense")), None);
    }
}
