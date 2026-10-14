// The rules for what may be run as an administrator: one local file of a runnable kind, checked before anything is launched
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Compiled on every platform so the rules are tested everywhere, but only the Windows backend acts on them. The check fails closed: a location that cannot be shown to be one existing, regular file with a runnable extension is refused, and nothing is launched.

use std::path::Path;

use crate::error::{MimeAppsError, Result};
use crate::target::Target;

/// The extensions (lower case, no dot) of what Windows can run with the `runas` verb: programs, scripts, installers and shortcuts.
pub const RUNNABLE_EXTENSIONS: [&str; 5] = ["exe", "bat", "cmd", "msi", "lnk"];

/// Whether the name ends in a runnable extension, whatever its case.
pub fn is_runnable_name(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(stem, extension)| {
        !stem.is_empty()
            && RUNNABLE_EXTENSIONS
                .iter()
                .any(|runnable| extension.eq_ignore_ascii_case(runnable))
    })
}

/// Checks that the locations are exactly one existing, regular, local file of a runnable kind, and returns it. `Empty` for none, `Unsupported` for several, a folder or another kind of file, and `InvalidUri` for a location that is not local, is gone, or is not a regular file (a link, a device, a folder).
pub fn check(targets: &[Target]) -> Result<&Target> {
    let target = match targets {
        [] => return Err(MimeAppsError::Empty),
        [target] => target,
        _ => return Err(MimeAppsError::Unsupported),
    };
    let invalid = || MimeAppsError::InvalidUri {
        uri: target.uri.clone(),
    };
    // A location with no local path is remote, in the Trash or an administrator's view: none of them is run from here.
    let path = target.path.as_deref().ok_or_else(invalid)?;
    if target.trailing_slash || !is_absolute_text(path) {
        return Err(invalid());
    }
    if !is_runnable_name(&target.name) {
        return Err(MimeAppsError::Unsupported);
    }
    // A link is refused, not followed: what would run is not what the listing showed.
    let on_disk = path
        .to_str()
        .and_then(windows_launch_path)
        .map(std::path::PathBuf::from);
    let metadata =
        std::fs::symlink_metadata(on_disk.as_deref().unwrap_or(path)).map_err(|_| invalid())?;
    if !metadata.file_type().is_file() {
        return Err(invalid());
    }
    Ok(target)
}

/// Whether the path is written as an absolute one: from a root, or from a drive (`C:\x`). Read from the text, as a `file:///C:/x` location parses to `/C:/x`, which `Path::is_absolute` does not accept on Windows.
fn is_absolute_text(path: &Path) -> bool {
    path.to_str().is_some_and(|text| {
        let bytes = text.as_bytes();
        text.starts_with('/')
            || (bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && matches!(bytes[2], b'\\' | b'/'))
    })
}

/// The path to hand to the shell, as a Windows drive path with backslashes (`C:\Tools\a.exe`), or `None` when the location is anything else: a share, a device or extended path, a path with `..` or a colon beyond the drive (an alternate data stream), a wildcard, or a control character. A `file:///C:/x` location parses to `/C:/x`, which is read as `C:\x`.
pub fn windows_launch_path(path: &str) -> Option<String> {
    let text = match path.as_bytes() {
        [b'/', letter, b':', b'/' | b'\\', ..] if letter.is_ascii_alphabetic() => &path[1..],
        _ => path,
    };
    let bytes = text.as_bytes();
    let drive = bytes.len() >= 4
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');
    if !drive || text[2..].contains([':', '?', '*', '"', '<', '>', '|']) {
        return None;
    }
    if text.chars().any(char::is_control) {
        return None;
    }
    let normal = text.replace('/', "\\");
    // No empty segment (a doubled separator) and no `.` or `..` segment.
    let mut segments = normal[3..].split('\\');
    let clean = segments.all(|segment| !segment.is_empty() && segment != "." && segment != "..");
    clean.then_some(normal)
}

/// The folder a launched file starts in: its own, as a Windows path.
pub fn windows_parent(launch_path: &str) -> Option<&str> {
    launch_path
        .rsplit_once('\\')
        .map(|(parent, _)| parent)
        .filter(|parent| !parent.is_empty())
        .map(|parent| {
            if parent.len() == 2 {
                &launch_path[..3]
            } else {
                parent
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target::parse;

    fn file(dir: &Path, name: &str) -> Target {
        let path = dir.join(name);
        std::fs::write(&path, "x").unwrap();
        parse(&path.display().to_string()).unwrap()
    }

    #[test]
    fn the_five_kinds_are_runnable_in_any_case() {
        for name in ["a.exe", "a.BAT", "a.Cmd", "a.msi", "a.LNK", "x.y.exe"] {
            assert!(is_runnable_name(name), "{name}");
        }
        for name in [
            "a.txt",
            "a",
            ".exe",
            "a.exe.txt",
            "a.exe ",
            "a.com",
            "a.ps1",
            "a.dll",
        ] {
            assert!(!is_runnable_name(name), "{name}");
        }
    }

    #[test]
    fn one_existing_runnable_file_passes() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["setup.exe", "run.BAT", "go.cmd", "pkg.msi", "link.lnk"] {
            let targets = [file(dir.path(), name)];
            assert_eq!(check(&targets).unwrap(), &targets[0], "{name}");
        }
    }

    #[test]
    fn none_or_several_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(check(&[]), Err(MimeAppsError::Empty));
        let both = [file(dir.path(), "a.exe"), file(dir.path(), "b.exe")];
        assert_eq!(check(&both), Err(MimeAppsError::Unsupported));
    }

    #[test]
    fn another_kind_of_file_is_refused_before_the_file_system_is_asked() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            check(&[file(dir.path(), "notes.txt")]),
            Err(MimeAppsError::Unsupported)
        );
        // Not even present: the extension is what is refused.
        assert_eq!(
            check(&[parse("/nowhere/a.ps1").unwrap()]),
            Err(MimeAppsError::Unsupported)
        );
    }

    #[test]
    fn a_missing_file_a_folder_and_a_folder_named_like_a_program_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let missing = parse(&dir.path().join("gone.exe").display().to_string()).unwrap();
        assert!(matches!(
            check(&[missing]),
            Err(MimeAppsError::InvalidUri { .. })
        ));
        let folder = dir.path().join("fake.exe");
        std::fs::create_dir(&folder).unwrap();
        let by_path = parse(&folder.display().to_string()).unwrap();
        assert!(matches!(
            check(&[by_path]),
            Err(MimeAppsError::InvalidUri { .. })
        ));
        let by_slash = parse(&format!("{}/", folder.display())).unwrap();
        assert!(matches!(
            check(&[by_slash]),
            Err(MimeAppsError::InvalidUri { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let real = file(dir.path(), "real.exe");
        let link = dir.path().join("link.exe");
        std::os::unix::fs::symlink(real.path.as_ref().unwrap(), &link).unwrap();
        let target = parse(&link.display().to_string()).unwrap();
        assert!(matches!(
            check(&[target]),
            Err(MimeAppsError::InvalidUri { .. })
        ));
    }

    #[test]
    fn remote_trash_and_administrator_locations_are_refused() {
        for uri in [
            "admin:///etc/a.exe",
            "trash:///a.exe",
            "sftp://host/home/u/a.exe",
            "smb://nas/share/setup.exe",
            "s3://bucket/a.exe",
        ] {
            assert!(
                matches!(
                    check(&[parse(uri).unwrap()]),
                    Err(MimeAppsError::InvalidUri { .. })
                ),
                "{uri}"
            );
        }
    }

    #[test]
    fn a_file_uri_is_a_local_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("my setup.exe");
        std::fs::write(&path, "x").unwrap();
        let uri = crate::target::path_to_uri(&path.display().to_string());
        assert!(check(&[parse(&uri).unwrap()]).is_ok());
    }

    #[test]
    fn a_launch_path_is_a_plain_drive_path() {
        assert_eq!(
            windows_launch_path("/C:/Tools/setup.exe").as_deref(),
            Some(r"C:\Tools\setup.exe")
        );
        assert_eq!(
            windows_launch_path(r"d:\x\a b.bat").as_deref(),
            Some(r"d:\x\a b.bat")
        );
        assert_eq!(
            windows_launch_path("C:/a.exe").as_deref(),
            Some(r"C:\a.exe")
        );
    }

    #[test]
    fn shares_devices_streams_and_traversal_have_no_launch_path() {
        for bad in [
            r"\\server\share\a.exe",
            "//server/share/a.exe",
            r"\\?\C:\a.exe",
            r"\\.\C:\a.exe",
            r"C:\dir\..\a.exe",
            r"C:\dir\.\a.exe",
            r"C:\dir\\a.exe",
            r"C:\a.exe:stream",
            r"C:\dir:x\a.exe",
            r"C:\wild*.exe",
            "C:\\a\u{7}.exe",
            "/usr/bin/a.exe",
            "relative.exe",
            r"C:a.exe",
            "C:/",
        ] {
            assert_eq!(windows_launch_path(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_start_folder_is_the_files_own() {
        assert_eq!(windows_parent(r"C:\Tools\a.exe"), Some(r"C:\Tools"));
        assert_eq!(windows_parent(r"C:\a.exe"), Some(r"C:\"));
    }
}
