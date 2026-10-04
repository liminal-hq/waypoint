// Locations inside an archive (`archive:{container URI}!/inner/path`), on any provider and nested.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::{OsStr, OsString};

use crate::segments::{self, Segment};
use crate::{ConnectionKey, PathError, VfsPath};

/// The scheme of a location inside an archive.
pub const ARCHIVE_SCHEME: &str = "archive";

/// Where something is inside an archive file. The container is the archive's own location in any
/// provider (a server, or another archive), so archives nest; `inner` is the path inside it.
///
/// The URI is `archive:` and the container's canonical URI, then `!` and the inner path. A
/// literal `!` in a name is always percent-encoded, so the last `!/` is the one that separates
/// the levels.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArchivePath {
    container: Box<VfsPath>,
    inner: Vec<Segment>,
}

/// Finds the `!` that ends the container: the last one followed by `/` or by nothing.
fn separator(body: &str) -> Option<usize> {
    body.rmatch_indices('!')
        .map(|(at, _)| at)
        .find(|&at| body[at + 1..].is_empty() || body[at + 1..].starts_with('/'))
}

impl ArchivePath {
    /// The top of the archive at `container`, which must be a file's location (it has a name)
    /// and not in the Trash.
    pub fn new(container: VfsPath) -> Result<Self, PathError> {
        if matches!(container, VfsPath::Trash(_)) {
            return Err(PathError::InvalidUri(
                "an archive in the Trash cannot be opened",
            ));
        }
        if container.file_name().is_none() {
            return Err(PathError::InvalidUri("an archive is a file, not a root"));
        }
        Ok(Self {
            container: Box::new(container),
            inner: Vec::new(),
        })
    }

    /// Whether `text` is written in the archive scheme, whatever follows.
    pub fn is_archive_uri(text: &str) -> bool {
        text.get(..ARCHIVE_SCHEME.len() + 1).is_some_and(|head| {
            head[..ARCHIVE_SCHEME.len()].eq_ignore_ascii_case(ARCHIVE_SCHEME) && head.ends_with(':')
        })
    }

    pub fn from_uri(uri: &str) -> Result<Self, PathError> {
        Self::parse(uri, false).map(|(path, _)| path)
    }

    /// Reads an archive location; leniently (typed text), the container may be a plain path and
    /// a missing `!/` means the top of the archive. Also says whether a password was dropped.
    pub(crate) fn parse(text: &str, lenient: bool) -> Result<(Self, bool), PathError> {
        if !Self::is_archive_uri(text) {
            let scheme = text
                .split(':')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            return Err(PathError::UnsupportedScheme(scheme));
        }
        let body = &text[ARCHIVE_SCHEME.len() + 1..];
        let (container, inner) = match separator(body) {
            Some(at) => (&body[..at], &body[at + 1..]),
            None if lenient => (body, ""),
            None => {
                return Err(PathError::InvalidUri(
                    "an archive location ends the archive with `!/`",
                ))
            }
        };
        let (container, had_password) = if lenient {
            VfsPath::parse_input_reporting(container)?
        } else {
            (VfsPath::from_uri(container)?, false)
        };
        let mut path = Self::new(container)?;
        path.inner = segments::parse_path(inner, lenient)?;
        Ok((path, had_password))
    }

    /// The archive file's own location.
    pub fn container(&self) -> &VfsPath {
        &self.container
    }

    /// The names inside the archive, as bytes.
    pub fn inner(&self) -> &[Vec<u8>] {
        &self.inner
    }

    pub fn is_root(&self) -> bool {
        self.inner.is_empty()
    }

    /// The containing folder inside the archive, or `None` at its top (where Up leaves the
    /// archive: see `VfsPath::parent`).
    pub fn parent(&self) -> Option<Self> {
        let (_, rest) = self.inner.split_last()?;
        Some(Self {
            container: self.container.clone(),
            inner: rest.to_vec(),
        })
    }

    pub fn join(&self, child: impl AsRef<OsStr>) -> Result<Self, PathError> {
        let child = segments::os_bytes(child.as_ref())?;
        Ok(Self {
            container: self.container.clone(),
            inner: segments::join(&self.inner, &child)?,
        })
    }

    pub fn file_name(&self) -> Option<OsString> {
        self.inner.last().map(|name| segments::os_string(name))
    }

    /// The login of the archive's container, when it is on a server.
    pub fn connection_key(&self) -> Option<ConnectionKey> {
        self.container.connection_key()
    }

    pub fn to_uri(&self) -> String {
        format!(
            "{ARCHIVE_SCHEME}:{}!{}",
            self.container.to_uri(),
            segments::render(&self.inner)
        )
    }

    /// The archive as people read it, then the path inside it: `/home/a/photos.zip › 2026`.
    pub fn display(&self) -> String {
        if self.inner.is_empty() {
            return self.container.display();
        }
        format!(
            "{} › {}",
            self.container.display(),
            &segments::display(&self.inner)[1..]
        )
    }

    /// The label of the archive's top breadcrumb: the archive's name.
    pub fn root_label(&self) -> String {
        self.container
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn an_archive_on_a_local_drive_round_trips() {
        let path =
            ArchivePath::from_uri("archive:file:///home/a/photos.zip!/2026/b%20c.jpg").unwrap();
        assert_eq!(path.container().to_uri(), "file:///home/a/photos.zip");
        assert_eq!(
            path.to_uri(),
            "archive:file:///home/a/photos.zip!/2026/b%20c.jpg"
        );
        assert_eq!(path.display(), "/home/a/photos.zip › 2026/b c.jpg");
        assert_eq!(path.root_label(), "photos.zip");
        let top = ArchivePath::from_uri("archive:file:///home/a/photos.zip!/").unwrap();
        assert!(top.is_root());
        assert_eq!(top.display(), "/home/a/photos.zip");
        assert_eq!(top.join("2026/b c.jpg").unwrap(), path);
    }

    #[test]
    fn archives_nest_and_live_on_servers() {
        let nested = "archive:archive:sftp://me@nas/srv/outer.tar!/inner.zip!/docs";
        let path = ArchivePath::from_uri(nested).unwrap();
        assert_eq!(path.to_uri(), nested);
        let VfsPath::Archive(outer) = path.container() else {
            panic!("the container is an archive");
        };
        assert_eq!(outer.container().to_uri(), "sftp://me@nas/srv/outer.tar");
        assert_eq!(path.connection_key().unwrap().as_str(), "sftp://me@nas");
        assert_eq!(
            path.display(),
            "sftp://me@nas/srv/outer.tar › inner.zip › docs"
        );
    }

    #[test]
    fn a_bang_in_a_name_is_encoded_and_never_a_separator() {
        let top = ArchivePath::new(VfsPath::parse_input("/tmp/wow!.zip").unwrap()).unwrap();
        let path = top.join("a!/b").unwrap();
        assert_eq!(path.to_uri(), "archive:file:///tmp/wow%21.zip!/a%21/b");
        assert_eq!(ArchivePath::from_uri(&path.to_uri()).unwrap(), path);
    }

    #[test]
    fn typed_text_may_name_the_archive_by_its_path() {
        let (path, had_password) = ArchivePath::parse("archive:/tmp/x.zip", true).unwrap();
        assert!(path.is_root() && !had_password);
        assert_eq!(path.to_uri(), "archive:file:///tmp/x.zip!/");
        let (path, had_password) =
            ArchivePath::parse("ARCHIVE:sftp://me:pw@h/x.zip!/a", true).unwrap();
        assert!(had_password);
        assert_eq!(path.to_uri(), "archive:sftp://me@h/x.zip!/a");
    }

    #[test]
    fn nonsense_is_refused() {
        for text in [
            "archive:file:///tmp/x.zip",
            "archive:trash:/abc!/",
            "archive:file:///!/",
            "archive:nonsense!/a",
            "archive:sftp://me:pw@h/x.zip!/",
            "archive:file:///tmp/x.zip!/a%2Fb",
        ] {
            assert!(ArchivePath::from_uri(text).is_err(), "{text}");
        }
    }

    #[test]
    fn walking_up_stops_at_the_top_of_the_archive() {
        let path = ArchivePath::from_uri("archive:file:///a.zip!/b/c").unwrap();
        let b = path.parent().unwrap();
        assert_eq!(b.to_uri(), "archive:file:///a.zip!/b");
        let top = b.parent().unwrap();
        assert!(top.is_root());
        assert_eq!(top.parent(), None);
        assert_eq!(path.file_name().unwrap(), "c");
    }
}
