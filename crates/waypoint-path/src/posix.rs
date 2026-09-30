// The Linux path rules, as pure functions over bytes so they are unit-tested on every platform.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// A POSIX path is a byte string: any byte but NUL and `/` may appear in a name, so none of this
// assumes UTF-8. Normalisation is lexical (it collapses `.`, `..` and repeated slashes without
// touching the file system), which is what a typed location wants; it can differ from the kernel's
// answer when a component is a symlink.

use crate::{encoding, PathError};

pub fn is_absolute(path: &[u8]) -> bool {
    path.first() == Some(&b'/')
}

/// The meaningful components of `path`, with empty and `.` components dropped and `..` resolved
/// against the components before it (and ignored at the root, as the kernel does).
fn resolve(path: &[u8], mut stack: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    for part in path.split(|&b| b == b'/') {
        match part {
            b"" | b"." => {}
            b".." => {
                stack.pop();
            }
            name => stack.push(name.to_vec()),
        }
    }
    stack
}

fn render(components: &[Vec<u8>]) -> Vec<u8> {
    if components.is_empty() {
        return b"/".to_vec();
    }
    let mut out = Vec::new();
    for part in components {
        out.push(b'/');
        out.extend_from_slice(part);
    }
    out
}

/// Normalises an absolute path: one leading slash, no trailing slash (except the root), and no `.`
/// or `..` components.
pub fn normalise(path: &[u8]) -> Result<Vec<u8>, PathError> {
    if path.is_empty() {
        return Err(PathError::Empty);
    }
    if path.contains(&0) {
        return Err(PathError::InteriorNul);
    }
    if !is_absolute(path) {
        return Err(PathError::NotAbsolute);
    }
    Ok(render(&resolve(path, Vec::new())))
}

/// Joins `child` onto the normalised absolute path `base`. An absolute `child` replaces `base`.
pub fn join(base: &[u8], child: &[u8]) -> Result<Vec<u8>, PathError> {
    if child.contains(&0) {
        return Err(PathError::InteriorNul);
    }
    if is_absolute(child) {
        return normalise(child);
    }
    Ok(render(&resolve(child, resolve(base, Vec::new()))))
}

/// The parent of a normalised absolute path, or `None` for the root.
pub fn parent(path: &[u8]) -> Option<Vec<u8>> {
    let components = resolve(path, Vec::new());
    if components.is_empty() {
        return None;
    }
    Some(render(&components[..components.len() - 1]))
}

/// The last component of a normalised absolute path, or `None` for the root.
pub fn file_name(path: &[u8]) -> Option<&[u8]> {
    let trimmed = path.strip_suffix(b"/").unwrap_or(path);
    let name = trimmed.rsplit(|&b| b == b'/').next()?;
    (!name.is_empty()).then_some(name)
}

/// The percent-encoded `file://` URI for a normalised absolute path.
pub fn to_uri(path: &[u8]) -> String {
    let mut out = String::from("file://");
    encoding::encode_into(&mut out, path, b"/");
    out
}

/// The normalised path a `file://` URI names. The host must be empty or `localhost`.
pub fn from_uri(rest: &str) -> Result<Vec<u8>, PathError> {
    let (host, path) = match rest.find('/') {
        Some(at) => rest.split_at(at),
        None => (rest, ""),
    };
    if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
        return Err(PathError::RemoteHost(host.to_owned()));
    }
    if path.contains(['?', '#']) {
        return Err(PathError::InvalidUri(
            "a query or fragment is not part of a path",
        ));
    }
    normalise(&encoding::decode(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(path: &str) -> String {
        String::from_utf8(normalise(path.as_bytes()).unwrap()).unwrap()
    }

    #[test]
    fn normalises_slashes_dots_and_dot_dot() {
        assert_eq!(n("/"), "/");
        assert_eq!(n("//a///b/"), "/a/b");
        assert_eq!(n("/a/./b/../c"), "/a/c");
        assert_eq!(n("/../.."), "/");
    }

    #[test]
    fn rejects_relative_empty_and_nul() {
        assert_eq!(normalise(b""), Err(PathError::Empty));
        assert_eq!(normalise(b"a/b"), Err(PathError::NotAbsolute));
        assert_eq!(normalise(b"/a\0b"), Err(PathError::InteriorNul));
    }

    #[test]
    fn joins_relative_and_absolute_children() {
        assert_eq!(join(b"/a", b"b/c").unwrap(), b"/a/b/c");
        assert_eq!(join(b"/a/b", b"../c").unwrap(), b"/a/c");
        assert_eq!(join(b"/a", b"/x/y").unwrap(), b"/x/y");
        assert_eq!(join(b"/", b"x").unwrap(), b"/x");
    }

    #[test]
    fn finds_parent_and_file_name() {
        assert_eq!(parent(b"/a/b"), Some(b"/a".to_vec()));
        assert_eq!(parent(b"/a"), Some(b"/".to_vec()));
        assert_eq!(parent(b"/"), None);
        assert_eq!(file_name(b"/a/b"), Some(&b"b"[..]));
        assert_eq!(file_name(b"/"), None);
    }

    #[test]
    fn keeps_non_utf8_names_through_a_uri() {
        let path = b"/tmp/caf\xe9 %#?".to_vec();
        let uri = to_uri(&path);
        assert_eq!(uri, "file:///tmp/caf%E9%20%25%23%3F");
        assert_eq!(
            from_uri(uri.strip_prefix("file://").unwrap()).unwrap(),
            path
        );
    }

    #[test]
    fn accepts_localhost_and_refuses_other_hosts() {
        assert_eq!(from_uri("localhost/a").unwrap(), b"/a");
        assert_eq!(from_uri("/a").unwrap(), b"/a");
        assert_eq!(
            from_uri("server/a"),
            Err(PathError::RemoteHost("server".to_owned()))
        );
        assert!(from_uri("/a?b").is_err());
        assert!(from_uri("").is_err());
    }
}
