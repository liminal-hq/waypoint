// The path of a location seen through the elevated helper: a local path that is read and changed as the administrator.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::{OsStr, OsString};

use crate::{FilePath, PathError};

/// The URI scheme of an elevated location.
pub const ELEVATED_SCHEME: &str = "admin";

/// A local path that is reached through the elevated helper: `admin:///etc` is `/etc` as the
/// helper sees it. It is the same path as the `file:` one with another scheme, so the tab's
/// location says that it is elevated and nothing else has to; leaving elevation is the same path
/// as a `FilePath`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElevatedPath(FilePath);

impl ElevatedPath {
    pub fn new(path: FilePath) -> Self {
        Self(path)
    }

    /// The same path as an ordinary local one: what leaving elevation navigates to, and what the
    /// helper acts on.
    pub fn file(&self) -> &FilePath {
        &self.0
    }

    pub fn into_file(self) -> FilePath {
        self.0
    }

    /// Whether `uri` is written in the elevated scheme, whatever follows.
    pub fn is_elevated_uri(uri: &str) -> bool {
        uri.get(..ELEVATED_SCHEME.len() + 1).is_some_and(|head| {
            head[..ELEVATED_SCHEME.len()].eq_ignore_ascii_case(ELEVATED_SCHEME)
                && head.ends_with(':')
        })
    }

    pub fn join(&self, child: impl AsRef<OsStr>) -> Result<Self, PathError> {
        self.0.join(child).map(Self)
    }

    pub fn parent(&self) -> Option<Self> {
        self.0.parent().map(Self)
    }

    pub fn file_name(&self) -> Option<OsString> {
        self.0.file_name()
    }

    pub fn is_root(&self) -> bool {
        self.0.is_root()
    }

    /// The path for people. It reads as the local path; the elevated mark is the tab's and the
    /// window's, not part of the name.
    pub fn display(&self) -> String {
        self.0.display()
    }

    /// The lossless URI: the `file:` one with the elevated scheme.
    pub fn to_uri(&self) -> String {
        let file = self.0.to_uri();
        let rest = file
            .strip_prefix("file:")
            .expect("a file URI starts with `file:`");
        format!("{ELEVATED_SCHEME}:{rest}")
    }

    /// Reads an elevated URI. It names a path on this machine, so a host is refused as it is in a
    /// `file:` URI.
    pub fn from_uri(uri: &str) -> Result<Self, PathError> {
        if !Self::is_elevated_uri(uri) {
            let scheme = uri.split(':').next().unwrap_or_default().to_owned();
            return Err(PathError::UnsupportedScheme(scheme));
        }
        FilePath::from_uri(&format!("file:{}", &uri[ELEVATED_SCHEME.len() + 1..])).map(Self)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn it_is_the_file_uri_with_another_scheme() {
        let path = ElevatedPath::from_uri("admin:///etc/a%20b").unwrap();
        assert_eq!(path.display(), "/etc/a b");
        assert_eq!(path.to_uri(), "admin:///etc/a%20b");
        assert_eq!(path.file().to_uri(), "file:///etc/a%20b");
        assert_eq!(ElevatedPath::new(path.file().clone()), path);
    }

    #[test]
    fn it_reads_the_scheme_in_any_case_and_refuses_the_rest() {
        assert!(ElevatedPath::from_uri("ADMIN:///etc").is_ok());
        assert_eq!(
            ElevatedPath::from_uri("file:///etc"),
            Err(PathError::UnsupportedScheme("file".to_owned()))
        );
        assert_eq!(
            ElevatedPath::from_uri("admin://host/etc"),
            Err(PathError::RemoteHost("host".to_owned()))
        );
        assert!(ElevatedPath::from_uri("admin:relative").is_err());
        assert!(ElevatedPath::is_elevated_uri("admin:///"));
        assert!(!ElevatedPath::is_elevated_uri("administrator:///"));
    }

    #[test]
    fn it_walks_like_a_local_path() {
        let root = ElevatedPath::from_uri("admin:///").unwrap();
        assert!(root.is_root());
        assert_eq!(root.parent(), None);
        let child = root.join("root").unwrap().join("a b").unwrap();
        assert_eq!(child.to_uri(), "admin:///root/a%20b");
        assert_eq!(child.file_name(), Some(OsString::from("a b")));
        assert_eq!(child.parent().unwrap().to_uri(), "admin:///root");
    }
}
