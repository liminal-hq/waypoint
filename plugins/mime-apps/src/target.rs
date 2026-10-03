// Reads the paths and URIs the plugin is given: the file name they end in, and where they are local
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use crate::error::{MimeAppsError, Result};

/// What one location the plugin was given is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The location as a URI, which is what an application is started with.
    pub uri: String,
    /// The last segment, percent-decoded: the name the type is guessed from. Empty for a root.
    pub name: String,
    /// The local path, for a `file://` URI or a plain path.
    pub path: Option<PathBuf>,
    /// The location ends in a slash, so it names a directory whatever the file system says.
    pub trailing_slash: bool,
}

impl Target {
    /// The extension of the name, without the dot and in lower case, when it has one.
    pub fn extension(&self) -> Option<String> {
        let (stem, extension) = self.name.rsplit_once('.')?;
        (!stem.is_empty() && !extension.is_empty()).then(|| extension.to_ascii_lowercase())
    }

    /// Whether the location is a directory: it says so, or it is local and the file system says so.
    pub fn is_directory(&self) -> bool {
        self.trailing_slash
            || self
                .path
                .as_ref()
                .is_some_and(|path| std::fs::metadata(path).is_ok_and(|meta| meta.is_dir()))
    }
}

/// Parses a path or a URI. A string with a URL scheme is a URI (`file:`, `smb:`, `sftp:`, …); one that starts with a slash, or with a drive letter and a colon, is a path; anything else is rejected, so a relative path is never resolved against the working directory by accident.
pub fn parse(input: &str) -> Result<Target> {
    let invalid = || MimeAppsError::InvalidUri {
        uri: input.to_string(),
    };
    let text = input.trim();
    if text.is_empty() {
        return Err(invalid());
    }
    match scheme_of(text) {
        Some(scheme) if scheme.eq_ignore_ascii_case("file") => {
            let rest = &text[scheme.len() + 1..];
            let rest = rest.strip_prefix("//").ok_or_else(invalid)?;
            // `file://host/path`: only the local host is a path here.
            let path_start = rest.find('/').ok_or_else(invalid)?;
            let host = &rest[..path_start];
            if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
                return Err(invalid());
            }
            let raw = rest[path_start..]
                .split(['?', '#'])
                .next()
                .unwrap_or_default();
            let path = decode(raw).ok_or_else(invalid)?;
            Ok(from_path(&path))
        }
        Some(scheme) => {
            let raw = text[scheme.len() + 1..]
                .split(['?', '#'])
                .next()
                .unwrap_or_default();
            let raw_path = raw
                .strip_prefix("//")
                .map(|rest| rest.find('/').map_or("", |index| &rest[index..]))
                .unwrap_or(raw);
            let trailing_slash = raw_path.ends_with('/');
            let last = raw_path
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or("");
            Ok(Target {
                uri: text.to_string(),
                name: decode(last).ok_or_else(invalid)?,
                path: None,
                trailing_slash,
            })
        }
        None if text.starts_with('/') || is_drive_path(text) => Ok(from_path(text)),
        None => Err(invalid()),
    }
}

fn from_path(path: &str) -> Target {
    let trailing_slash = path.len() > 1 && path.ends_with(['/', '\\']);
    let name = path
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_string();
    Target {
        uri: path_to_uri(path),
        name,
        path: Some(PathBuf::from(path)),
        trailing_slash,
    }
}

fn is_drive_path(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

/// The scheme of a URI: letters, digits, `+`, `-` and `.`, starting with a letter, at least two characters long (so a drive letter is not one), followed by a colon.
fn scheme_of(text: &str) -> Option<&str> {
    let end = text.find(':')?;
    let scheme = &text[..end];
    let mut chars = scheme.chars();
    let first = chars.next()?;
    (scheme.len() >= 2
        && first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')))
    .then_some(scheme)
}

/// `file://` plus the path, percent-encoded except for unreserved characters and `/`.
pub fn path_to_uri(path: &str) -> String {
    let mut uri = String::from("file://");
    if is_drive_path(path) {
        uri.push('/');
    }
    for byte in path.replace('\\', "/").bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/' | b':') {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}

/// Decodes `%XX` escapes; `None` when an escape is malformed or the result is not UTF-8.
pub fn decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_uri_gives_its_decoded_name_and_path() {
        let target = parse("file:///home/u/My%20Pictures/cat.PNG").unwrap();
        assert_eq!(target.name, "cat.PNG");
        assert_eq!(
            target.path,
            Some(PathBuf::from("/home/u/My Pictures/cat.PNG"))
        );
        assert_eq!(target.extension().as_deref(), Some("png"));
        assert!(!target.trailing_slash);
    }

    #[test]
    fn a_plain_path_becomes_an_encoded_file_uri() {
        let target = parse("/tmp/a b#c.txt").unwrap();
        assert_eq!(target.uri, "file:///tmp/a%20b%23c.txt");
        assert_eq!(target.name, "a b#c.txt");
    }

    #[test]
    fn localhost_is_local_and_other_hosts_are_not_paths() {
        assert!(parse("file://localhost/etc/hosts").unwrap().path.is_some());
        assert!(matches!(
            parse("file://server/share/a.txt"),
            Err(MimeAppsError::InvalidUri { .. })
        ));
    }

    #[test]
    fn a_remote_uri_has_no_path_and_loses_its_query() {
        let target = parse("smb://nas/media/Holiday%20Photos/clip.mp4?x=1#frag").unwrap();
        assert_eq!(target.name, "clip.mp4");
        assert_eq!(target.path, None);
        assert_eq!(
            target.uri,
            "smb://nas/media/Holiday%20Photos/clip.mp4?x=1#frag"
        );
    }

    #[test]
    fn a_trailing_slash_names_a_directory() {
        let target = parse("sftp://host/home/u/docs/").unwrap();
        assert_eq!(target.name, "docs");
        assert!(target.trailing_slash && target.is_directory());
        assert!(parse("/usr/").unwrap().is_directory());
    }

    #[test]
    fn the_file_system_decides_for_a_local_path_without_a_slash() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = parse(&tmp.path().display().to_string()).unwrap();
        assert!(dir.is_directory());
        let file = tmp.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(!parse(&file.display().to_string()).unwrap().is_directory());
    }

    #[test]
    fn a_name_with_no_extension_or_only_a_dot_prefix_has_none() {
        assert_eq!(parse("/a/Makefile").unwrap().extension(), None);
        assert_eq!(parse("/a/.bashrc").unwrap().extension(), None);
        assert_eq!(
            parse("/a/archive.tar.gz").unwrap().extension().as_deref(),
            Some("gz")
        );
    }

    #[test]
    fn relative_paths_empty_text_and_bad_escapes_are_rejected() {
        for bad in [
            "",
            "  ",
            "relative/path.txt",
            "file:///a/%zz",
            "file:/no/slashes",
        ] {
            assert!(
                matches!(parse(bad), Err(MimeAppsError::InvalidUri { .. })),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_drive_path_is_a_path_not_a_scheme() {
        let target = parse(r"C:\Users\u\notes.txt").unwrap();
        assert_eq!(target.name, "notes.txt");
        assert_eq!(target.uri, "file:///C:/Users/u/notes.txt");
    }
}
