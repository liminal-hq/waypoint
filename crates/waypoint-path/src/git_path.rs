// Locations in a revision of a local Git repository (`git+file:///repo!/inner?rev=main`).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::{OsStr, OsString};

use crate::encoding::encode_into;
use crate::segments::{self, Segment};
use crate::{FilePath, PathError};

/// The scheme of a location in a Git revision.
pub const GIT_SCHEME: &str = "git+file";

/// A path in one revision of a local repository, browsed read-only. `rev` is a branch, a tag or a
/// commit; `None` follows `HEAD`. The status overlay on ordinary folders is not a location.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GitPath {
    repo: FilePath,
    rev: Option<String>,
    inner: Vec<Segment>,
}

fn invalid(why: &'static str) -> PathError {
    PathError::InvalidUri(why)
}

fn check_rev(rev: String) -> Result<Option<String>, PathError> {
    if rev.is_empty() || rev.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(invalid("a revision is a name or a commit, without spaces"));
    }
    Ok((rev != "HEAD").then_some(rev))
}

impl GitPath {
    /// The top of `repo` at `rev` (`None` or `HEAD` follows `HEAD`).
    pub fn new(repo: FilePath, rev: Option<String>) -> Result<Self, PathError> {
        Ok(Self {
            repo,
            rev: match rev {
                Some(rev) => check_rev(rev)?,
                None => None,
            },
            inner: Vec::new(),
        })
    }

    /// Whether `text` is written in the Git scheme, whatever follows.
    pub fn is_git_uri(text: &str) -> bool {
        text.get(..GIT_SCHEME.len() + 1).is_some_and(|head| {
            head[..GIT_SCHEME.len()].eq_ignore_ascii_case(GIT_SCHEME) && head.ends_with(':')
        })
    }

    pub fn from_uri(uri: &str) -> Result<Self, PathError> {
        Self::parse(uri, false)
    }

    /// Reads a Git location. Without `!/` the text names the top of the repository; leniently
    /// (typed text) `?` is part of a name unless `?rev=` follows.
    pub(crate) fn parse(text: &str, lenient: bool) -> Result<Self, PathError> {
        if !Self::is_git_uri(text) {
            let scheme = text
                .split(':')
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            return Err(PathError::UnsupportedScheme(scheme));
        }
        // `file:///repo!/inner?rev=…`.
        let body = &text["git+".len()..];
        let (body, rev) = if lenient {
            match body.rfind("?rev=") {
                Some(at) => (&body[..at], Some(&body[at + 5..])),
                None => (body, None),
            }
        } else {
            match body.split_once('?') {
                Some((body, query)) => (
                    body,
                    Some(
                        query
                            .strip_prefix("rev=")
                            .filter(|rev| !rev.contains('&'))
                            .ok_or(invalid("a Git location takes only `?rev=`"))?,
                    ),
                ),
                None => (body, None),
            }
        };
        let (repo, inner) = match body
            .rmatch_indices('!')
            .map(|(at, _)| at)
            .find(|&at| body[at + 1..].is_empty() || body[at + 1..].starts_with('/'))
        {
            Some(at) => (&body[..at], &body[at + 1..]),
            None => (body, ""),
        };
        let repo = FilePath::from_uri(repo)?;
        let rev = match rev {
            Some(rev) => {
                let bytes = segments::decode_text(rev, lenient)?;
                Some(String::from_utf8(bytes).map_err(|_| invalid("a revision is text"))?)
            }
            None => None,
        };
        let mut path = Self::new(repo, rev)?;
        path.inner = segments::parse_path(inner, lenient)?;
        Ok(path)
    }

    /// The repository's working folder.
    pub fn repo(&self) -> &FilePath {
        &self.repo
    }

    /// The revision, or `None` for `HEAD`.
    pub fn rev(&self) -> Option<&str> {
        self.rev.as_deref()
    }

    /// The names inside the revision, as bytes.
    pub fn inner(&self) -> &[Vec<u8>] {
        &self.inner
    }

    pub fn is_root(&self) -> bool {
        self.inner.is_empty()
    }

    /// The containing folder in the revision, or `None` at its top (where Up goes to the
    /// repository's own folder: see `VfsPath::parent`).
    pub fn parent(&self) -> Option<Self> {
        let (_, rest) = self.inner.split_last()?;
        Some(Self {
            inner: rest.to_vec(),
            ..self.clone()
        })
    }

    pub fn join(&self, child: impl AsRef<OsStr>) -> Result<Self, PathError> {
        let child = segments::os_bytes(child.as_ref())?;
        Ok(Self {
            inner: segments::join(&self.inner, &child)?,
            ..self.clone()
        })
    }

    pub fn file_name(&self) -> Option<OsString> {
        self.inner.last().map(|name| segments::os_string(name))
    }

    pub fn to_uri(&self) -> String {
        let mut out = format!(
            "git+{}!{}",
            self.repo.to_uri(),
            segments::render(&self.inner)
        );
        if let Some(rev) = &self.rev {
            out.push_str("?rev=");
            encode_into(&mut out, rev.as_bytes(), b"/");
        }
        out
    }

    fn rev_label(&self) -> &str {
        self.rev.as_deref().unwrap_or("HEAD")
    }

    /// `/home/a/waypoint @ main › src`.
    pub fn display(&self) -> String {
        let mut out = format!("{} @ {}", self.repo.display(), self.rev_label());
        if !self.inner.is_empty() {
            out.push_str(" › ");
            out.push_str(&segments::display(&self.inner)[1..]);
        }
        out
    }

    /// The label of the revision's top breadcrumb: `waypoint @ main`.
    pub fn root_label(&self) -> String {
        let name = self
            .repo
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.repo.display());
        format!("{name} @ {}", self.rev_label())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_revision_round_trips() {
        let path =
            GitPath::from_uri("git+file:///home/a/waypoint!/src/lib.rs?rev=release/1.0").unwrap();
        assert_eq!(path.repo().display(), "/home/a/waypoint");
        assert_eq!(path.rev(), Some("release/1.0"));
        assert_eq!(
            path.to_uri(),
            "git+file:///home/a/waypoint!/src/lib.rs?rev=release/1.0"
        );
        assert_eq!(
            path.display(),
            "/home/a/waypoint @ release/1.0 › src/lib.rs"
        );
        assert_eq!(path.root_label(), "waypoint @ release/1.0");
    }

    #[test]
    fn head_is_the_default_and_the_bare_repository_is_its_top() {
        let path = GitPath::from_uri("GIT+FILE:///repo").unwrap();
        assert!(path.is_root());
        assert_eq!(path.rev(), None);
        assert_eq!(path.to_uri(), "git+file:///repo!/");
        assert_eq!(
            GitPath::from_uri("git+file:///repo!/?rev=HEAD").unwrap(),
            path
        );
        assert_eq!(path.display(), "/repo @ HEAD");
    }

    #[test]
    fn walking_inside_the_revision() {
        let top = GitPath::new(FilePath::parse("/r").unwrap(), Some("v2".into())).unwrap();
        let file = top.join("a b/c#d").unwrap();
        assert_eq!(file.to_uri(), "git+file:///r!/a%20b/c%23d?rev=v2");
        assert_eq!(GitPath::from_uri(&file.to_uri()).unwrap(), file);
        assert_eq!(file.parent().unwrap().parent().unwrap(), top);
        assert_eq!(top.parent(), None);
        assert_eq!(file.file_name().unwrap(), "c#d");
    }

    #[test]
    fn nonsense_is_refused() {
        for text in [
            "git+file:///r!/?rev=",
            "git+file:///r!/?rev=a%20b",
            "git+file:///r!/?branch=x",
            "git+file://host/r",
            "git+https://example.com/r",
        ] {
            assert!(GitPath::from_uri(text).is_err(), "{text}");
        }
    }
}
