// The scheme-shaped path: a location in any provider, of which only `file` exists so far.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_protocol::Location;

use crate::trash_path::{TrashPath, TRASH_SCHEME};
use crate::{FilePath, PathError};

/// A path in one of Waypoint's providers. The scheme picks the provider; `sftp`, `smb`, `davs`,
/// `s3`, `git+file` and archive paths join this enum as their providers are built.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VfsPath {
    File(FilePath),
    /// The Trash, and the items in it.
    Trash(TrashPath),
}

impl VfsPath {
    /// The URI scheme, which names the provider that owns this path.
    pub fn scheme(&self) -> &'static str {
        match self {
            VfsPath::File(_) => "file",
            VfsPath::Trash(_) => TRASH_SCHEME,
        }
    }

    /// Parses a lossless URI. `file` and `trash` are implemented; any other scheme is reported by
    /// name.
    pub fn from_uri(uri: &str) -> Result<Self, PathError> {
        if TrashPath::is_trash_uri(uri) {
            return TrashPath::from_uri(uri).map(VfsPath::Trash);
        }
        FilePath::from_uri(uri).map(VfsPath::File)
    }

    /// Parses text from the path bar or Go to…: a URI when it has a scheme (`file://…`), otherwise a
    /// native absolute path. A Windows drive letter (`C:\a`) is a path, not a one-letter scheme.
    pub fn parse_input(text: &str) -> Result<Self, PathError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(PathError::Empty);
        }
        if TrashPath::is_trash_uri(text) {
            return Self::from_uri(text);
        }
        match text.find("://") {
            Some(at)
                if at > 1
                    && text[..at]
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) =>
            {
                Self::from_uri(text)
            }
            _ => FilePath::parse(text).map(VfsPath::File),
        }
    }

    pub fn join(&self, child: impl AsRef<std::ffi::OsStr>) -> Result<Self, PathError> {
        match self {
            VfsPath::File(path) => path.join(child).map(VfsPath::File),
            VfsPath::Trash(path) => child
                .as_ref()
                .to_str()
                .ok_or(PathError::Unrepresentable)
                .and_then(|id| path.join(id))
                .map(VfsPath::Trash),
        }
    }

    pub fn parent(&self) -> Option<Self> {
        match self {
            VfsPath::File(path) => path.parent().map(VfsPath::File),
            VfsPath::Trash(path) => path.parent().map(VfsPath::Trash),
        }
    }

    pub fn display(&self) -> String {
        match self {
            VfsPath::File(path) => path.display(),
            VfsPath::Trash(path) => path.display(),
        }
    }

    pub fn to_uri(&self) -> String {
        match self {
            VfsPath::File(path) => path.to_uri(),
            VfsPath::Trash(path) => path.to_uri(),
        }
    }

    pub fn to_location(&self) -> Location {
        Location::new(self.display(), self.to_uri())
    }

    /// Reads the lossless form of a location. The `display` text is ignored.
    pub fn from_location(location: &Location) -> Result<Self, PathError> {
        Self::from_uri(&location.uri)
    }
}

impl From<FilePath> for VfsPath {
    fn from(path: FilePath) -> Self {
        VfsPath::File(path)
    }
}

impl From<&VfsPath> for Location {
    fn from(path: &VfsPath) -> Self {
        path.to_location()
    }
}

impl TryFrom<&Location> for VfsPath {
    type Error = PathError;

    fn try_from(location: &Location) -> Result<Self, PathError> {
        VfsPath::from_location(location)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn typed_input_is_a_path_or_a_uri() {
        assert_eq!(VfsPath::parse_input(" /a/b ").unwrap().display(), "/a/b");
        assert_eq!(
            VfsPath::parse_input("file:///a/b%20c").unwrap().display(),
            "/a/b c"
        );
        assert_eq!(
            VfsPath::parse_input("sftp://h/a"),
            Err(PathError::UnsupportedScheme("sftp".to_owned()))
        );
        assert_eq!(VfsPath::parse_input("  "), Err(PathError::Empty));
    }

    #[test]
    fn converts_to_and_from_a_location() {
        let path = VfsPath::parse_input("/tmp/a b").unwrap();
        let location: Location = (&path).into();
        assert_eq!(location, Location::new("/tmp/a b", "file:///tmp/a%20b"));
        assert_eq!(VfsPath::try_from(&location).unwrap(), path);
        assert_eq!(path.scheme(), "file");
        assert_eq!(path.parent().unwrap().display(), "/tmp");
    }

    #[test]
    fn the_trash_is_a_scheme_of_its_own() {
        let root = VfsPath::parse_input("trash:/").unwrap();
        assert_eq!(root.scheme(), "trash");
        assert_eq!(root.to_location(), Location::new("Trash", "trash:/"));
        let item = root.join("a|b c").unwrap();
        assert_eq!(item.to_uri(), "trash:/a%7Cb%20c");
        assert_eq!(VfsPath::from_location(&item.to_location()).unwrap(), item);
        assert_eq!(item.parent(), Some(root));
    }
}
