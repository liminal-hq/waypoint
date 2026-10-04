// Turning the names an archive stores into safe names to browse by, and saying what was unsafe.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsString;

/// Why a name an archive stores cannot be used as it is written. The name the person sees and
/// every path built from it is the safe one; this says what was changed, so an extraction can
/// refuse or skip the entry instead of trusting the raw name (never write outside a destination).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnsafeName {
    /// The name starts at the root (`/etc/passwd`) or a drive (`C:\Windows`).
    Absolute,
    /// A component is `..`, so the name climbs out of the folder it is extracted into.
    Traversal,
    /// The name holds a NUL or another control character.
    ControlCharacters,
    /// The entry sits below a name the archive made a link or a file, so extracting it would write
    /// through the link.
    ThroughLink,
}

/// A name taken apart into safe components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SafeName {
    pub components: Vec<Vec<u8>>,
    pub unsafe_name: Option<UnsafeName>,
}

/// What a `..` component is shown as: a name that is a name, so it browses, and that reads as what
/// it was.
const DOT_DOT: &[u8] = b"%2E%2E";

/// Splits a stored name into components that can be joined onto a folder without leaving it.
///
/// `backslash` makes `\` a separator too (zip archives made on Windows write it). Empty and `.`
/// components are dropped, a leading root or drive is removed, `..` becomes `%2E%2E`, and
/// control characters become `\u{fffd}`. The result may be empty (the entry names the archive's own
/// top).
pub(crate) fn sanitise(raw: &[u8], backslash: bool) -> SafeName {
    let mut flag = None;
    let mut components: Vec<Vec<u8>> = Vec::new();
    let is_separator = |byte: u8| byte == b'/' || (backslash && byte == b'\\');
    let mut rest = raw;
    // A drive letter (`C:`, `c:/`) is a root too.
    if rest.len() >= 2 && rest[0].is_ascii_alphabetic() && rest[1] == b':' && backslash {
        flag = Some(UnsafeName::Absolute);
        rest = &rest[2..];
    }
    if rest.first().is_some_and(|&byte| is_separator(byte)) {
        flag = Some(UnsafeName::Absolute);
    }
    for part in rest.split(|&byte| is_separator(byte)) {
        match part {
            b"" | b"." => {}
            b".." => {
                flag.get_or_insert(UnsafeName::Traversal);
                components.push(DOT_DOT.to_vec());
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
    SafeName {
        components,
        unsafe_name: flag,
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
    if target.first() == Some(&b'/') {
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
        assert_eq!(parts, ["%2E%2E", "%2E%2E", "etc", "passwd"]);
        assert_eq!(flag, Some(UnsafeName::Traversal));
        let (parts, flag) = names("a/../b", false);
        assert_eq!(parts, ["a", "%2E%2E", "b"]);
        assert_eq!(flag, Some(UnsafeName::Traversal));
    }

    #[test]
    fn roots_and_drives_are_removed() {
        let (parts, flag) = names("/etc/passwd", false);
        assert_eq!(parts, ["etc", "passwd"]);
        assert_eq!(flag, Some(UnsafeName::Absolute));
        let (parts, flag) = names("C:\\Windows\\system32", true);
        assert_eq!(parts, ["Windows", "system32"]);
        assert_eq!(flag, Some(UnsafeName::Absolute));
        // Without backslash rules a colon is an ordinary name character.
        assert_eq!(names("C:\\x", false), (vec!["C:\\x".into()], None));
    }

    #[test]
    fn backslash_separates_only_where_asked() {
        assert_eq!(names("a\\b", true).0, ["a", "b"]);
        assert_eq!(names("a\\b", false).0, ["a\\b"]);
        let (parts, flag) = names("..\\x", true);
        assert_eq!(parts, ["%2E%2E", "x"]);
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
    }
}
