// The scheme-shaped path: a location in any provider, local, in the Trash, on a server, inside an
// archive or in a Git revision.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsString;

use waypoint_protocol::Location;

use crate::archive_path::{ArchivePath, ARCHIVE_SCHEME};
use crate::git_path::{GitPath, GIT_SCHEME};
use crate::remote_path::{ConnectionKey, RemotePath, RemoteScheme};
use crate::trash_path::{TrashPath, TRASH_SCHEME};
use crate::{FilePath, PathError};

/// A path in one of Waypoint's providers. The scheme picks the provider.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VfsPath {
    File(FilePath),
    /// The Trash, and the items in it.
    Trash(TrashPath),
    /// A folder or file on a server: `sftp`, `smb`, `dav`, `davs` or `s3`.
    Remote(RemotePath),
    /// A path inside an archive file.
    Archive(ArchivePath),
    /// A path in a revision of a local Git repository.
    Git(GitPath),
}

/// The scheme of `text` when it is written `scheme://…`. A Windows drive letter (`C:\a`) is a
/// path, not a one-letter scheme.
fn hierarchical_scheme(text: &str) -> Option<&str> {
    let at = text.find("://")?;
    let scheme = &text[..at];
    (at > 1
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')))
    .then_some(scheme)
}

impl VfsPath {
    /// The URI scheme, which names the provider that owns this path.
    pub fn scheme(&self) -> &'static str {
        match self {
            VfsPath::File(_) => "file",
            VfsPath::Trash(_) => TRASH_SCHEME,
            VfsPath::Remote(path) => path.scheme().as_str(),
            VfsPath::Archive(_) => ARCHIVE_SCHEME,
            VfsPath::Git(_) => GIT_SCHEME,
        }
    }

    /// Parses a lossless URI strictly: a malformed one, or one holding a password, is refused.
    pub fn from_uri(uri: &str) -> Result<Self, PathError> {
        if TrashPath::is_trash_uri(uri) {
            return TrashPath::from_uri(uri).map(VfsPath::Trash);
        }
        if ArchivePath::is_archive_uri(uri) {
            return ArchivePath::from_uri(uri).map(VfsPath::Archive);
        }
        if GitPath::is_git_uri(uri) {
            return GitPath::from_uri(uri).map(VfsPath::Git);
        }
        if hierarchical_scheme(uri).is_some_and(|scheme| RemoteScheme::from_name(scheme).is_some())
        {
            return RemotePath::from_uri(uri).map(VfsPath::Remote);
        }
        FilePath::from_uri(uri).map(VfsPath::File)
    }

    /// Parses text from the path bar or Go to…: a URI when it has a scheme, otherwise a native
    /// absolute path. A password written in a server address is dropped.
    pub fn parse_input(text: &str) -> Result<Self, PathError> {
        Self::parse_input_reporting(text).map(|(path, _)| path)
    }

    /// `parse_input`, also saying whether a password was written in the text and dropped, so the
    /// path bar can say that it was not kept.
    pub fn parse_input_reporting(text: &str) -> Result<(Self, bool), PathError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(PathError::Empty);
        }
        if TrashPath::is_trash_uri(text) {
            return Self::from_uri(text).map(|path| (path, false));
        }
        if ArchivePath::is_archive_uri(text) {
            return ArchivePath::parse(text, true)
                .map(|(path, had_password)| (VfsPath::Archive(path), had_password));
        }
        if GitPath::is_git_uri(text) {
            return GitPath::parse(text, true).map(|path| (VfsPath::Git(path), false));
        }
        match hierarchical_scheme(text) {
            Some(scheme) if RemoteScheme::from_name(scheme).is_some() => {
                RemotePath::parse(text, true)
                    .map(|parsed| (VfsPath::Remote(parsed.path), parsed.had_password))
            }
            Some(_) => Self::from_uri(text).map(|path| (path, false)),
            None => FilePath::parse(text).map(|path| (VfsPath::File(path), false)),
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
            VfsPath::Remote(path) => path.join(child).map(VfsPath::Remote),
            VfsPath::Archive(path) => path.join(child).map(VfsPath::Archive),
            VfsPath::Git(path) => path.join(child).map(VfsPath::Git),
        }
    }

    /// The containing location. Up from the top of an archive goes to the folder that holds the
    /// archive, and Up from the top of a revision to the repository's own folder.
    pub fn parent(&self) -> Option<Self> {
        match self {
            VfsPath::File(path) => path.parent().map(VfsPath::File),
            VfsPath::Trash(path) => path.parent().map(VfsPath::Trash),
            VfsPath::Remote(path) => path.parent().map(VfsPath::Remote),
            VfsPath::Archive(path) => match path.parent() {
                Some(parent) => Some(VfsPath::Archive(parent)),
                None => path.container().parent(),
            },
            VfsPath::Git(path) => match path.parent() {
                Some(parent) => Some(VfsPath::Git(parent)),
                None => Some(VfsPath::File(path.repo().clone())),
            },
        }
    }

    /// The last name, or `None` at a root (a Trash item's name is its id; the top of an archive or
    /// a revision has none of its own).
    pub fn file_name(&self) -> Option<OsString> {
        match self {
            VfsPath::File(path) => path.file_name(),
            VfsPath::Trash(path) => path.id().map(OsString::from),
            VfsPath::Remote(path) => path.file_name(),
            VfsPath::Archive(path) => path.file_name(),
            VfsPath::Git(path) => path.file_name(),
        }
    }

    /// The text of this location's breadcrumb: its name, or the label of the root it is.
    pub fn label(&self) -> String {
        if let Some(name) = self.file_name() {
            return name.to_string_lossy().into_owned();
        }
        match self {
            VfsPath::File(path) => path.display(),
            VfsPath::Trash(path) => path.display(),
            VfsPath::Remote(path) => path.root_label(),
            VfsPath::Archive(path) => path.root_label(),
            VfsPath::Git(path) => path.root_label(),
        }
    }

    /// The login this location belongs to: a server's, or the server holding an archive. `None`
    /// for anything that needs no connection.
    pub fn connection_key(&self) -> Option<ConnectionKey> {
        match self {
            VfsPath::Remote(path) => Some(path.connection_key()),
            VfsPath::Archive(path) => path.connection_key(),
            VfsPath::File(_) | VfsPath::Trash(_) | VfsPath::Git(_) => None,
        }
    }

    pub fn display(&self) -> String {
        match self {
            VfsPath::File(path) => path.display(),
            VfsPath::Trash(path) => path.display(),
            VfsPath::Remote(path) => path.display(),
            VfsPath::Archive(path) => path.display(),
            VfsPath::Git(path) => path.display(),
        }
    }

    pub fn to_uri(&self) -> String {
        match self {
            VfsPath::File(path) => path.to_uri(),
            VfsPath::Trash(path) => path.to_uri(),
            VfsPath::Remote(path) => path.to_uri(),
            VfsPath::Archive(path) => path.to_uri(),
            VfsPath::Git(path) => path.to_uri(),
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
            VfsPath::parse_input("ftp://h/a"),
            Err(PathError::UnsupportedScheme("ftp".to_owned()))
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

    #[test]
    fn every_scheme_is_read_and_written_back() {
        for (uri, scheme) in [
            ("file:///a", "file"),
            ("trash:/x", "trash"),
            ("sftp://me@h/a", "sftp"),
            ("smb://h/share", "smb"),
            ("dav://h/", "dav"),
            ("davs://h/a", "davs"),
            ("s3://bucket/k", "s3"),
            ("archive:file:///a.zip!/b", "archive"),
            ("git+file:///r!/src?rev=main", "git+file"),
        ] {
            let path = VfsPath::from_uri(uri).unwrap();
            assert_eq!(path.scheme(), scheme, "{uri}");
            assert_eq!(path.to_uri(), uri);
            assert_eq!(VfsPath::from_location(&path.to_location()).unwrap(), path);
        }
    }

    #[test]
    fn a_typed_password_is_reported_and_dropped() {
        let (path, had) = VfsPath::parse_input_reporting("sftp://me:s3cret@h/x").unwrap();
        assert!(had);
        let location = path.to_location();
        assert!(!location.uri.contains("s3cret") && !location.display.contains("s3cret"));
        assert_eq!(
            VfsPath::from_uri("sftp://me:s3cret@h/x"),
            Err(PathError::PasswordInUri)
        );
        let (_, had) = VfsPath::parse_input_reporting("sftp://me@h/x").unwrap();
        assert!(!had);
    }

    #[test]
    fn up_leaves_an_archive_and_a_revision() {
        let archive = VfsPath::from_uri("archive:sftp://h/srv/a.zip!/").unwrap();
        assert_eq!(archive.parent().unwrap().to_uri(), "sftp://h/srv");
        assert_eq!(archive.label(), "a.zip");
        assert_eq!(archive.connection_key().unwrap().as_str(), "sftp://h");
        let revision = VfsPath::from_uri("git+file:///home/a/r!/?rev=v1").unwrap();
        assert_eq!(revision.parent().unwrap().to_uri(), "file:///home/a/r");
        assert_eq!(revision.label(), "r @ v1");
        assert_eq!(revision.connection_key(), None);
    }

    #[test]
    fn labels_name_the_entry_or_its_root() {
        let label = |uri: &str| VfsPath::from_uri(uri).unwrap().label();
        assert_eq!(label("file:///"), "/");
        assert_eq!(label("file:///a/b%20c"), "b c");
        assert_eq!(label("trash:/"), "Trash");
        assert_eq!(label("sftp://me@h:2222/"), "me@h:2222");
        assert_eq!(label("smb://h/Share"), "Share");
    }
}
