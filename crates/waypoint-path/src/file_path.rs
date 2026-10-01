// An absolute, normalised path on the local file system, under the running platform's rules.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use waypoint_protocol::Location;

#[cfg(unix)]
use crate::posix;
#[cfg(windows)]
use crate::windows;
use crate::PathError;

/// How a platform compares file names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseRule {
    /// Names differing only in case are different files (Linux).
    Sensitive,
    /// Names differing only in case are the same file (Windows).
    Insensitive,
}

impl CaseRule {
    /// The rule of the platform this is compiled for.
    pub const NATIVE: CaseRule = if cfg!(windows) {
        CaseRule::Insensitive
    } else {
        CaseRule::Sensitive
    };
}

/// An absolute local path, normalised lexically (no `.`, `..`, repeated or trailing separators).
///
/// The real bytes are kept (a Linux name need not be UTF-8), `display` is the lossy form for people,
/// and [`FilePath::to_uri`] is the lossless, percent-encoded `file://` form that locations carry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FilePath(PathBuf);

#[cfg(unix)]
fn native_bytes(path: &OsStr) -> &[u8] {
    std::os::unix::ffi::OsStrExt::as_bytes(path)
}

#[cfg(unix)]
fn from_native_bytes(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(<OsString as std::os::unix::ffi::OsStringExt>::from_vec(
        bytes,
    ))
}

#[cfg(windows)]
fn native_str(path: &OsStr) -> Result<&str, PathError> {
    path.to_str().ok_or(PathError::Unrepresentable)
}

impl FilePath {
    /// Parses text typed by a person or read from configuration into an absolute local path.
    pub fn parse(text: impl AsRef<OsStr>) -> Result<Self, PathError> {
        Self::from_os_str(text.as_ref())
    }

    /// Adopts a native path, which must be absolute.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, PathError> {
        Self::from_os_str(path.as_ref().as_os_str())
    }

    #[cfg(unix)]
    fn from_os_str(text: &OsStr) -> Result<Self, PathError> {
        Ok(Self(from_native_bytes(posix::normalise(native_bytes(
            text,
        ))?)))
    }

    #[cfg(windows)]
    fn from_os_str(text: &OsStr) -> Result<Self, PathError> {
        Self::from_win(windows::parse(native_str(text)?)?)
    }

    #[cfg(windows)]
    fn from_win(path: windows::WinPath) -> Result<Self, PathError> {
        if !path.is_absolute() {
            return Err(PathError::NotAbsolute);
        }
        Ok(Self(PathBuf::from(path.to_native_string())))
    }

    #[cfg(windows)]
    fn to_win(&self) -> windows::WinPath {
        // Every `FilePath` was built from a parsed, absolute, valid Unicode path.
        windows::parse(&self.0.to_string_lossy()).expect("a FilePath always re-parses")
    }

    /// The path to hand to `std::fs`.
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }

    /// Joins a relative name or path (`..` allowed) onto this path. An absolute `child` replaces it;
    /// on Windows a child that stays relative to another drive is rejected.
    pub fn join(&self, child: impl AsRef<OsStr>) -> Result<Self, PathError> {
        let child = child.as_ref();
        #[cfg(unix)]
        {
            let joined = posix::join(native_bytes(self.0.as_os_str()), native_bytes(child))?;
            Ok(Self(from_native_bytes(joined)))
        }
        #[cfg(windows)]
        {
            let joined = self.to_win().join(&windows::parse(native_str(child)?)?);
            Self::from_win(joined)
        }
    }

    /// The containing folder, or `None` at a root.
    pub fn parent(&self) -> Option<Self> {
        #[cfg(unix)]
        {
            posix::parent(native_bytes(self.0.as_os_str())).map(|p| Self(from_native_bytes(p)))
        }
        #[cfg(windows)]
        {
            self.to_win().parent().and_then(|p| Self::from_win(p).ok())
        }
    }

    /// The last component, or `None` at a root.
    pub fn file_name(&self) -> Option<OsString> {
        #[cfg(unix)]
        {
            posix::file_name(native_bytes(self.0.as_os_str()))
                .map(|n| <OsString as std::os::unix::ffi::OsStringExt>::from_vec(n.to_vec()))
        }
        #[cfg(windows)]
        {
            self.to_win().file_name().map(OsString::from)
        }
    }

    /// Whether this is a filesystem root (`/`, `C:\`, `\\server\share\`).
    pub fn is_root(&self) -> bool {
        self.parent().is_none()
    }

    /// The path for people (path bar, titles): lossy for names that are not valid Unicode, and a
    /// verbatim Windows path shows as the ordinary path it names.
    pub fn display(&self) -> String {
        #[cfg(unix)]
        {
            self.0.to_string_lossy().into_owned()
        }
        #[cfg(windows)]
        {
            self.to_win().to_display_string()
        }
    }

    /// The percent-encoded `file://` URI, lossless for any file name.
    pub fn to_uri(&self) -> String {
        #[cfg(unix)]
        {
            posix::to_uri(native_bytes(self.0.as_os_str()))
        }
        #[cfg(windows)]
        {
            windows::to_uri(&self.to_win()).expect("a FilePath is always absolute")
        }
    }

    /// Reads a `file://` URI back to the path it names.
    pub fn from_uri(uri: &str) -> Result<Self, PathError> {
        let rest = strip_file_scheme(uri)?;
        #[cfg(unix)]
        {
            Ok(Self(from_native_bytes(posix::from_uri(rest)?)))
        }
        #[cfg(windows)]
        {
            Self::from_win(windows::from_uri(rest)?)
        }
    }

    /// The location the frontend carries for this path.
    pub fn to_location(&self) -> Location {
        Location::new(self.display(), self.to_uri())
    }

    /// Reads the lossless form of a location. The `display` text is ignored.
    pub fn from_location(location: &Location) -> Result<Self, PathError> {
        Self::from_uri(&location.uri)
    }

    /// Whether `self` and `other` name the same file under the platform's case rule (ignoring
    /// symlinks and hard links, which only the file system can judge).
    pub fn same_as(&self, other: &Self) -> bool {
        self.fold_key() == other.fold_key()
    }

    /// A key equal for paths that are the same file under the platform's case rule, for use in hash
    /// maps keyed by path.
    pub fn fold_key(&self) -> OsString {
        #[cfg(unix)]
        {
            self.0.as_os_str().to_owned()
        }
        #[cfg(windows)]
        {
            OsString::from(self.to_win().fold_key())
        }
    }
}

fn strip_file_scheme(uri: &str) -> Result<&str, PathError> {
    let Some(colon) = uri.find(':') else {
        return Err(PathError::InvalidUri("there is no scheme"));
    };
    let (scheme, rest) = uri.split_at(colon);
    if !scheme.eq_ignore_ascii_case("file") {
        return Err(PathError::UnsupportedScheme(scheme.to_ascii_lowercase()));
    }
    rest.strip_prefix("://")
        .ok_or(PathError::InvalidUri("a file URI starts with `file://`"))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStrExt;

    #[test]
    fn parses_joins_and_walks_up() {
        let home = FilePath::parse("/home//me/./").unwrap();
        assert_eq!(home.display(), "/home/me");
        let doc = home.join("Documents/../Music").unwrap();
        assert_eq!(doc.display(), "/home/me/Music");
        assert_eq!(doc.file_name().unwrap(), "Music");
        assert_eq!(doc.parent().unwrap(), home);
        assert!(FilePath::parse("/").unwrap().is_root());
        assert!(FilePath::parse("relative").is_err());
    }

    #[test]
    fn linux_names_are_case_sensitive() {
        assert_eq!(CaseRule::NATIVE, CaseRule::Sensitive);
        let a = FilePath::parse("/tmp/A").unwrap();
        let b = FilePath::parse("/tmp/a").unwrap();
        assert!(!a.same_as(&b));
        assert!(a.same_as(&a.clone()));
    }

    #[test]
    fn round_trips_a_non_utf8_name_through_a_location() {
        let raw = OsStr::from_bytes(b"/tmp/caf\xe9/\xff\xfe name");
        let path = FilePath::parse(raw).unwrap();
        let location = path.to_location();
        assert_eq!(location.uri, "file:///tmp/caf%E9/%FF%FE%20name");
        assert!(location.display.contains('\u{fffd}'));
        let back = FilePath::from_location(&location).unwrap();
        assert_eq!(back.as_path().as_os_str().as_bytes(), raw.as_bytes());
    }

    #[test]
    fn reads_uris_with_any_scheme_case_and_refuses_others() {
        assert_eq!(FilePath::from_uri("FILE:///a").unwrap().display(), "/a");
        assert_eq!(
            FilePath::from_uri("sftp://host/a"),
            Err(PathError::UnsupportedScheme("sftp".to_owned()))
        );
        assert!(FilePath::from_uri("/a").is_err());
        assert!(FilePath::from_uri("file:/a").is_err());
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    fn parses_joins_and_compares_case_insensitively() {
        assert_eq!(CaseRule::NATIVE, CaseRule::Insensitive);
        let home = FilePath::parse(r"c:/Users//Me/.").unwrap();
        assert_eq!(home.display(), r"C:\Users\Me");
        let other = FilePath::parse(r"C:\USERS\me").unwrap();
        assert!(home.same_as(&other));
        assert_eq!(home.join("Docs").unwrap().parent().unwrap(), home);
        assert!(FilePath::parse(r"C:\").unwrap().is_root());
        assert!(FilePath::parse(r"C:foo").is_err());
    }

    #[test]
    fn round_trips_a_long_path_through_a_location() {
        let path = FilePath::parse(r"\\?\C:\very\long").unwrap();
        let location = path.to_location();
        assert_eq!(location.display, r"C:\very\long");
        assert_eq!(FilePath::from_location(&location).unwrap(), path);
    }
}
