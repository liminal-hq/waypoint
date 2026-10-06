// Turning the names an archive stores into safe names to browse by, and saying what was unsafe.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsString;

pub use waypoint_vfs::UnsafeName;

/// A name taken apart into safe components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SafeName {
    pub components: Vec<Vec<u8>>,
    pub unsafe_name: Option<UnsafeName>,
    /// The whole stored name is one component (it had a `..`): there is no folder to open.
    pub flat: bool,
}

/// What a name that is only `..` is shown as: a name that is a name, and reads as what it was.
const DOT_DOT: &[u8] = waypoint_vfs::FLAT_DOT_DOT.as_bytes();

/// What stands for `/` in the flat name of an entry that climbs out with `..` (U+2215, DIVISION
/// SLASH), so the name reads as it was stored but is one name, not a path.
const SLASH_SHOWN: char = waypoint_vfs::FLAT_SLASH;

/// Splits a stored name into components that can be joined onto a folder without leaving it.
///
/// `backslash` makes `\` a separator too (zip archives made on Windows write it). Empty and `.`
/// components are dropped, a leading root or a drive followed by a separator is removed, and
/// control characters become `\u{fffd}`. A name with a `..` in it is not split at all: it is one
/// name that reads as stored, with `\u{2215}` for each separator, so it is never a path and never
/// a folder that opens (a name that is only `..` is `%2E%2E`). A drive letter and a colon with a
/// name straight after (`a:b.txt`, drive-relative on Windows) is kept as stored and flagged. The
/// result may be empty (the entry names the archive's own top).
pub(crate) fn sanitise(raw: &[u8], backslash: bool) -> SafeName {
    let mut flag = None;
    let mut components: Vec<Vec<u8>> = Vec::new();
    let mut flat = false;
    let is_separator = |byte: u8| byte == b'/' || (backslash && byte == b'\\');
    let mut rest = raw;
    // A drive letter (`C:`, `c:/`) is a root too.
    if rest.len() >= 2 && rest[0].is_ascii_alphabetic() && rest[1] == b':' && backslash {
        flag = Some(UnsafeName::Absolute);
        // `C:\x` and `C:/x` lose the drive; `a:b.txt` is a drive-relative name on Windows, so it
        // is flagged but listed as stored.
        if rest.len() == 2 || is_separator(rest[2]) {
            rest = &rest[2..];
        }
    }
    if rest.first().is_some_and(|&byte| is_separator(byte)) {
        flag = Some(UnsafeName::Absolute);
    }
    for part in rest.split(|&byte| is_separator(byte)) {
        match part {
            b"" | b"." => {}
            b".." => {
                flat = true;
                flag.get_or_insert(UnsafeName::Traversal);
                components.push(b"..".to_vec());
            }
            _ => {
                if part.iter().any(|&byte| byte < 0x20 || byte == 0x7f) {
                    flag.get_or_insert(UnsafeName::ControlCharacters);
                    let mut clean = Vec::with_capacity(part.len());
                    for &byte in part {
                        if byte < 0x20 || byte == 0x7f {
                            clean.extend_from_slice("\u{fffd}".as_bytes());
                        } else {
                            clean.push(byte);
                        }
                    }
                    components.push(clean);
                } else {
                    components.push(part.to_vec());
                }
            }
        }
    }
    if flat {
        let mut joined = Vec::new();
        for (at, part) in components.iter().enumerate() {
            if at > 0 {
                joined.extend_from_slice(SLASH_SHOWN.to_string().as_bytes());
            }
            joined.extend_from_slice(part);
        }
        if joined == b".." {
            joined = DOT_DOT.to_vec();
        }
        components = vec![joined];
    }
    SafeName {
        components,
        unsafe_name: flag,
        flat,
    }
}

/// A component as the `OsString` a provider reports. On Windows a name that is not Unicode is
/// shown lossily, as nothing there can hold it.
pub(crate) fn os_name(bytes: &[u8]) -> OsString {
    #[cfg(unix)]
    {
        std::os::unix::ffi::OsStringExt::from_vec(bytes.to_vec())
    }
    #[cfg(not(unix))]
    {
        OsString::from(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// The bytes of a name a caller gave.
pub(crate) fn name_bytes(name: &std::ffi::OsStr) -> Vec<u8> {
    #[cfg(unix)]
    {
        std::os::unix::ffi::OsStrExt::as_bytes(name).to_vec()
    }
    #[cfg(not(unix))]
    {
        name.to_string_lossy().into_owned().into_bytes()
    }
}

/// Resolves the text a link holds against the folder the link is in, within the archive. `None`
/// when the target is absolute or climbs above the top of the archive: such a link points outside
/// and is shown as broken.
pub(crate) fn resolve_link_target(folder: &[Vec<u8>], target: &[u8]) -> Option<Vec<Vec<u8>>> {
    let drive = target.len() >= 2 && target[0].is_ascii_alphabetic() && target[1] == b':';
    if target.first() == Some(&b'/') || target.contains(&b'\\') || drive {
        return None;
    }
    let mut out = folder.to_vec();
    for part in target.split(|&byte| byte == b'/') {
        match part {
            b"" | b"." => {}
            b".." => {
                out.pop()?;
            }
            _ => out.push(part.to_vec()),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(raw: &str, backslash: bool) -> (Vec<String>, Option<UnsafeName>) {
        let safe = sanitise(raw.as_bytes(), backslash);
        (
            safe.components
                .iter()
                .map(|part| String::from_utf8_lossy(part).into_owned())
                .collect(),
            safe.unsafe_name,
        )
    }

    #[test]
    fn an_ordinary_name_is_unchanged() {
        assert_eq!(
            names("a/b/c.txt", false),
            (vec!["a".into(), "b".into(), "c.txt".into()], None)
        );
        assert_eq!(names("dir/", false), (vec!["dir".into()], None));
        assert_eq!(names("./a//b/./c", false).0, ["a", "b", "c"]);
        assert_eq!(names("./", false).0, Vec::<String>::new());
    }

    #[test]
    fn traversal_is_defanged_and_flagged() {
        let (parts, flag) = names("../../etc/passwd", false);
        assert_eq!(parts, ["..\u{2215}..\u{2215}etc\u{2215}passwd"]);
        assert_eq!(flag, Some(UnsafeName::Traversal));
        let (parts, flag) = names("a/../b", false);
        assert_eq!(parts, ["a\u{2215}..\u{2215}b"]);
        assert_eq!(flag, Some(UnsafeName::Traversal));
        // Alone, `..` must not be a folder that opens, nor a component that is `..`.
        assert_eq!(names("..", false).0, ["%2E%2E"]);
        assert_eq!(names("../", false).0, ["%2E%2E"]);
        assert!(sanitise(b"../x", false).flat);
    }

    #[test]
    fn roots_and_drives_are_removed() {
        let (parts, flag) = names("/etc/passwd", false);
        assert_eq!(parts, ["etc", "passwd"]);
        assert_eq!(flag, Some(UnsafeName::Absolute));
        let (parts, flag) = names("C:\\Windows\\system32", true);
        assert_eq!(parts, ["Windows", "system32"]);
        assert_eq!(flag, Some(UnsafeName::Absolute));
        // A colon straight after the letter is drive-relative on Windows: flagged, shown as stored.
        let (parts, flag) = names("a:b.txt", true);
        assert_eq!(parts, ["a:b.txt"]);
        assert_eq!(flag, Some(UnsafeName::Absolute));
        // Without backslash rules a colon is an ordinary name character.
        assert_eq!(names("C:\\x", false), (vec!["C:\\x".into()], None));
    }

    #[test]
    fn backslash_separates_only_where_asked() {
        assert_eq!(names("a\\b", true).0, ["a", "b"]);
        assert_eq!(names("a\\b", false).0, ["a\\b"]);
        let (parts, flag) = names("..\\x", true);
        assert_eq!(parts, ["..\u{2215}x"]);
        assert_eq!(flag, Some(UnsafeName::Traversal));
    }

    #[test]
    fn control_characters_are_replaced() {
        let safe = sanitise(b"ab\0cd/e\nf", false);
        assert_eq!(safe.unsafe_name, Some(UnsafeName::ControlCharacters));
        assert_eq!(safe.components[0], "ab\u{fffd}cd".as_bytes());
        assert_eq!(safe.components[1], "e\u{fffd}f".as_bytes());
    }

    #[test]
    fn links_resolve_inside_the_archive_only() {
        let folder = vec![b"a".to_vec(), b"b".to_vec()];
        assert_eq!(
            resolve_link_target(&folder, b"../c"),
            Some(vec![b"a".to_vec(), b"c".to_vec()])
        );
        assert_eq!(resolve_link_target(&folder, b"../../../x"), None);
        assert_eq!(resolve_link_target(&folder, b"/etc"), None);
        assert_eq!(resolve_link_target(&folder, b"C:\\x"), None);
        assert_eq!(resolve_link_target(&folder, b"..\\x"), None);
    }
}
