// The platform-independent parts of the Windows backend: path and id mapping, dates, error codes and refusals
//
// They live outside `windows.rs` so they are unit-tested on every platform, and so the COM code is only the thin part that needs a Windows machine.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![cfg_attr(not(any(windows, test)), allow(dead_code))]

use crate::error::TrashError;

/// Seconds between 1601-01-01 (the `FILETIME` epoch) and 1970-01-01.
const FILETIME_UNIX_OFFSET_SECONDS: i64 = 11_644_473_600;

/// Converts a `FILETIME` (100 ns ticks since 1601, as one 64-bit number) to seconds since the Unix epoch.
pub fn filetime_to_unix(ticks: u64) -> i64 {
    (ticks / 10_000_000) as i64 - FILETIME_UNIX_OFFSET_SECONDS
}

/// Whether an item trashed at `deleted_at` is `older_than_days` days old or more at `now` (both Unix seconds).
pub fn is_expired(deleted_at: i64, now: i64, older_than_days: u32) -> bool {
    deleted_at <= now - i64::from(older_than_days) * 86_400
}

/// Turns forward slashes into backslashes and drops trailing separators, except after a drive (`C:\`).
pub fn normalise(path: &str) -> String {
    let mut text = path.replace('/', "\\");
    while text.len() > 3 && text.ends_with('\\') {
        text.pop();
    }
    text
}

/// Whether two Windows paths name the same thing: the same text, ignoring case and a trailing separator.
pub fn same_path(a: &str, b: &str) -> bool {
    normalise(a).to_lowercase() == normalise(b).to_lowercase()
}

/// Splits `C:\a\b` into (`C:\a`, `b`). `None` for a drive root or a bare name.
pub fn split_parent(path: &str) -> Option<(String, String)> {
    let path = normalise(path);
    let cut = path.rfind('\\')?;
    let (parent, name) = (&path[..cut], &path[cut + 1..]);
    if name.is_empty() {
        return None;
    }
    // `C:` is not a folder; `C:\` is.
    let parent = if parent.len() == 2 && parent.ends_with(':') {
        format!("{parent}\\")
    } else {
        parent.to_string()
    };
    Some((parent, name.to_string()))
}

/// Joins the Recycle Bin's "original location" (a folder) and an item's name.
pub fn join_original(location: &str, name: &str) -> String {
    if location.is_empty() {
        return name.to_string();
    }
    if location.ends_with('\\') {
        format!("{location}{name}")
    } else {
        format!("{location}\\{name}")
    }
}

/// The name an item had. The Recycle Bin's display name is the item's whole original path, and Explorer hides known extensions in it, but the item's file in the bin (`$R1A2B3C.txt`) keeps the original extension, so put it back when the display name lacks it.
pub fn original_name(display: &str, bin_path: &str) -> String {
    let display = display.rsplit('\\').next().unwrap_or(display);
    let bin_name = bin_path.rsplit('\\').next().unwrap_or(bin_path);
    let extension = match bin_name.rfind('.') {
        Some(dot) if dot > 0 && dot + 1 < bin_name.len() => &bin_name[dot..],
        _ => return display.to_string(),
    };
    if display.to_lowercase().ends_with(&extension.to_lowercase()) {
        display.to_string()
    } else {
        format!("{display}{extension}")
    }
}

/// Why the plugin will not trash a path on Windows: a drive root, the user's profile folder or one of its parents, or something in a Recycle Bin. `None` when the path is fine.
pub fn refusal(path: &str, profile: Option<&str>) -> Option<&'static str> {
    let path = normalise(path);
    if split_parent(&path).is_none() {
        return Some("a drive or a root cannot be trashed");
    }
    let lower = path.to_lowercase();
    if lower.split('\\').any(|part| part == "$recycle.bin") {
        return Some("the Recycle Bin cannot be trashed");
    }
    if let Some(profile) = profile {
        let profile = normalise(profile).to_lowercase();
        if profile == lower || profile.starts_with(&format!("{}\\", lower.trim_end_matches('\\'))) {
            return Some("the user's profile folder and its parents cannot be trashed");
        }
    }
    None
}

/// Maps a Win32 error code (or the code inside an `HRESULT_FROM_WIN32`) to a typed error.
pub fn error_from_win32(code: u32, message: &str) -> TrashError {
    match code {
        2 | 3 => TrashError::NotFound,
        5 | 19 => TrashError::PermissionDenied,
        _ => TrashError::Io {
            message: message.to_string(),
        },
    }
}

/// Extracts the Win32 code of an `HRESULT` of the `HRESULT_FROM_WIN32` form (`0x8007xxxx`).
pub fn win32_code_of_hresult(hresult: i32) -> Option<u32> {
    let bits = hresult as u32;
    (bits >> 16 == 0x8007).then_some(bits & 0xFFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetimes_become_unix_seconds() {
        assert_eq!(filetime_to_unix(116_444_736_000_000_000), 0);
        assert_eq!(
            filetime_to_unix(116_444_736_000_000_000 + 86_400 * 10_000_000),
            86_400
        );
        // 2026-10-01T12:00:00Z
        assert_eq!(
            filetime_to_unix((1_790_856_000 + 11_644_473_600) * 10_000_000),
            1_790_856_000
        );
        assert!(filetime_to_unix(0) < 0);
    }

    #[test]
    fn expiry_counts_whole_days_and_includes_the_boundary() {
        let now = 100 * 86_400;
        assert!(is_expired(now - 30 * 86_400, now, 30));
        assert!(!is_expired(now - 30 * 86_400 + 1, now, 30));
        assert!(is_expired(now, now, 0));
        assert!(!is_expired(now + 5, now, 0));
    }

    #[test]
    fn paths_are_normalised_and_compared_without_case() {
        assert_eq!(normalise("C:/Users/me/"), "C:\\Users\\me");
        assert_eq!(normalise("C:\\"), "C:\\");
        assert!(same_path("c:\\users\\ME", "C:/Users/me/"));
        assert!(!same_path("C:\\a", "C:\\b"));
    }

    #[test]
    fn paths_split_into_folder_and_name() {
        assert_eq!(
            split_parent("C:\\Users\\me\\a.txt"),
            Some(("C:\\Users\\me".into(), "a.txt".into()))
        );
        assert_eq!(split_parent("C:\\a"), Some(("C:\\".into(), "a".into())));
        assert_eq!(split_parent("C:\\"), None);
        assert_eq!(split_parent("name"), None);
        assert_eq!(
            split_parent("\\\\server\\share\\f"),
            Some(("\\\\server\\share".into(), "f".into()))
        );
    }

    #[test]
    fn the_original_location_and_name_join() {
        assert_eq!(
            join_original("C:\\Users\\me", "a.txt"),
            "C:\\Users\\me\\a.txt"
        );
        assert_eq!(join_original("C:\\", "a.txt"), "C:\\a.txt");
        assert_eq!(join_original("", "a.txt"), "a.txt");
    }

    #[test]
    fn a_hidden_extension_is_put_back() {
        let bin = "C:\\$Recycle.Bin\\S-1-5-21-1\\$R1A2B3C.txt";
        assert_eq!(original_name("notes", bin), "notes.txt");
        assert_eq!(original_name("notes.txt", bin), "notes.txt");
        assert_eq!(original_name("NOTES.TXT", bin), "NOTES.TXT");
        assert_eq!(
            original_name("folder", "C:\\$Recycle.Bin\\S-1\\$R1A2B3C"),
            "folder"
        );
        assert_eq!(original_name("x", "C:\\$Recycle.Bin\\S-1\\$R1A2B3C."), "x");
        // The shell gives the whole original path as the display name.
        assert_eq!(original_name("C:\\Users\\me\\notes", bin), "notes.txt");
        assert_eq!(
            original_name("C:\\Users\\me\\a folder", "C:\\$Recycle.Bin\\S-1\\$R9"),
            "a folder"
        );
    }

    #[test]
    fn what_must_not_be_trashed_is_refused() {
        let profile = Some("C:\\Users\\me");
        assert!(refusal("C:\\", profile).is_some());
        assert!(refusal("C:\\Users\\me", profile).is_some());
        assert!(refusal("c:/users/ME/", profile).is_some());
        assert!(refusal("C:\\Users", profile).is_some());
        assert!(refusal("C:\\Users\\me\\Documents\\a.txt", profile).is_none());
        assert!(refusal("C:\\Users\\mellow", profile).is_none());
        assert!(refusal("C:\\$Recycle.Bin\\S-1\\$R1", profile).is_some());
        assert!(refusal("D:\\a", None).is_none());
    }

    #[test]
    fn win32_codes_map_to_typed_errors() {
        assert_eq!(error_from_win32(2, "x"), TrashError::NotFound);
        assert_eq!(error_from_win32(3, "x"), TrashError::NotFound);
        assert_eq!(error_from_win32(5, "x"), TrashError::PermissionDenied);
        assert!(
            matches!(error_from_win32(32, "in use"), TrashError::Io { message } if message == "in use")
        );
        assert_eq!(win32_code_of_hresult(0x8007_0005_u32 as i32), Some(5));
        assert_eq!(win32_code_of_hresult(0x8000_4005_u32 as i32), None);
        assert_eq!(win32_code_of_hresult(0), None);
    }
}
