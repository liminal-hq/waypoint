// Names in a tree are bytes; the listing wants native strings.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsString;

/// A name as a native string: exact on Unix, lossy elsewhere for bytes that are not UTF-8.
pub(crate) fn os_string(name: &[u8]) -> OsString {
    #[cfg(unix)]
    {
        <OsString as std::os::unix::ffi::OsStringExt>::from_vec(name.to_vec())
    }
    #[cfg(not(unix))]
    {
        OsString::from(String::from_utf8_lossy(name).into_owned())
    }
}

/// The bytes of a native name, for matching against a tree.
pub(crate) fn os_bytes(name: &std::ffi::OsStr) -> Vec<u8> {
    #[cfg(unix)]
    {
        std::os::unix::ffi::OsStrExt::as_bytes(name).to_vec()
    }
    #[cfg(not(unix))]
    {
        name.to_string_lossy().into_owned().into_bytes()
    }
}

/// Resolves a symlink's text against the folder holding it, inside one tree: the components of
/// where it points, or `None` when it points outside the tree (an absolute target, or enough `..`
/// to leave the top), which a tree cannot follow.
pub(crate) fn resolve_in_tree(folder: &[Vec<u8>], target: &[u8]) -> Option<Vec<Vec<u8>>> {
    if target.first() == Some(&b'/') || target.is_empty() {
        return None;
    }
    let mut parts: Vec<Vec<u8>> = folder.to_vec();
    for part in target.split(|&b| b == b'/') {
        match part {
            b"" | b"." => {}
            b".." => {
                parts.pop()?;
            }
            name => parts.push(name.to_vec()),
        }
    }
    Some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(parts: &[&str]) -> Vec<Vec<u8>> {
        parts.iter().map(|p| p.as_bytes().to_vec()).collect()
    }

    #[test]
    fn a_link_resolves_against_its_folder() {
        assert_eq!(
            resolve_in_tree(&folder(&["a", "b"]), b"../c/d"),
            Some(folder(&["a", "c", "d"]))
        );
        assert_eq!(
            resolve_in_tree(&folder(&["a"]), b"./x//y/"),
            Some(folder(&["a", "x", "y"]))
        );
    }

    #[test]
    fn a_link_that_leaves_the_tree_does_not_resolve() {
        assert_eq!(resolve_in_tree(&folder(&["a"]), b"../../x"), None);
        assert_eq!(resolve_in_tree(&folder(&["a"]), b"/etc/passwd"), None);
        assert_eq!(resolve_in_tree(&folder(&[]), b""), None);
    }
}
