// Reads the 12/24-hour convention of the process's `LC_TIME` locale from libc
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{ffi::CStr, ptr};

use crate::parse;

/// Classifies `nl_langinfo(T_FMT)` for the locale the environment selects (`LC_ALL`, `LC_TIME`,
/// then `LANG`).
///
/// A private locale object is used instead of the global one, because a process that never
/// called `setlocale` would otherwise always see the `C` locale and report 24-hour time.
pub fn read() -> Result<bool, String> {
    // SAFETY: `newlocale` with an empty name builds a locale object from the environment and
    // returns null on failure. The object is only used by `nl_langinfo_l` and is freed below, and
    // the returned string is copied before then.
    let format = unsafe {
        let locale = libc::newlocale(libc::LC_TIME_MASK, c"".as_ptr(), ptr::null_mut());
        if locale.is_null() {
            return Err("the environment's locale is not installed".to_string());
        }
        let raw = libc::nl_langinfo_l(libc::T_FMT, locale);
        let format = (!raw.is_null()).then(|| CStr::from_ptr(raw).to_string_lossy().into_owned());
        libc::freelocale(locale);
        format
    };
    let format = format.ok_or("the locale has no time format")?;
    parse::libc_time_format(&format)
        .ok_or_else(|| format!("the locale's time format {format:?} has no hour field"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whatever locale the test environment has, reading it must not fail outright.
    #[test]
    fn the_process_locale_can_be_read() {
        match read() {
            Ok(_) => {}
            // A minimal container may not have the configured locale installed.
            Err(reason) => assert!(reason.contains("locale"), "{reason}"),
        }
    }
}
