// Turning typed text into locations, and locations into breadcrumbs: the path logic of the path bar
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;

use waypoint_path::{FilePath, PathError, VfsPath};
use waypoint_protocol::{Location, VfsError};

use crate::model::{Breadcrumb, LocationInfo};

fn invalid(input: &str) -> VfsError {
    VfsError::InvalidLocation {
        input: input.to_owned(),
    }
}

/// The scheme of `text` when it is written as a URI (`sftp://host/x`), as the path bar treats it.
/// A Windows drive letter (`C:\a`) is a path, not a one-letter scheme.
fn scheme_of(text: &str) -> Option<&str> {
    let at = text.find("://")?;
    let scheme = &text[..at];
    (at > 1
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')))
    .then_some(scheme)
}

/// Turns text the person typed into a `Location`: an absolute path, `~` or `~/…` (under `home`),
/// a path relative to `base`, or a `file://` URI. Another scheme is `Unsupported`; empty,
/// malformed or NUL-containing text is `InvalidLocation`. Nothing is read from disk, so a
/// location that does not exist parses fine and is reported when its listing opens.
pub fn parse_location(input: &str, base: &Location, home: &Path) -> Result<Location, VfsError> {
    let text = input.trim();
    if text.is_empty() || text.contains('\0') {
        return Err(invalid(input));
    }
    if let Some(scheme) = scheme_of(text) {
        if !scheme.eq_ignore_ascii_case("file") {
            return Err(VfsError::Unsupported {
                what: scheme.to_ascii_lowercase(),
            });
        }
        return VfsPath::from_uri(text)
            .map(|path| path.to_location())
            .map_err(|_| invalid(input));
    }
    let tilde = text == "~" || text.starts_with("~/") || (cfg!(windows) && text.starts_with("~\\"));
    let path: Result<FilePath, PathError> = if tilde {
        let rest = text[1..].trim_start_matches(['/', '\\']);
        FilePath::from_path(home).and_then(|home| {
            if rest.is_empty() {
                Ok(home)
            } else {
                home.join(rest)
            }
        })
    } else {
        // An absolute `text` replaces the base, a relative one is resolved against it, and `.` and
        // `..` are folded away either way. A base that is not a local path can still anchor
        // absolute text.
        match FilePath::from_location(base) {
            Ok(base) => base.join(text),
            Err(_) => FilePath::parse(text),
        }
    };
    path.map(|path| path.to_location())
        .map_err(|_| invalid(input))
}

/// The parent and the breadcrumb segments of a location, from its root to itself. The root segment
/// is labelled as the platform writes it (`/`, `C:\`, `\\server\share\`).
pub fn describe_location(location: &Location) -> Result<LocationInfo, VfsError> {
    let path = FilePath::from_location(location).map_err(|_| invalid(&location.uri))?;
    let mut segments = Vec::new();
    let mut current = Some(path);
    while let Some(here) = current {
        let label = match here.file_name() {
            Some(name) => name.to_string_lossy().into_owned(),
            None => here.display(),
        };
        let next = here.parent();
        segments.push(Breadcrumb {
            label,
            location: here.to_location(),
        });
        current = next;
    }
    segments.reverse();
    let parent = match segments.len() {
        0 | 1 => None,
        n => Some(segments[n - 2].location.clone()),
    };
    Ok(LocationInfo { parent, segments })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn at(path: &str) -> Location {
        FilePath::parse(path).unwrap().to_location()
    }

    fn parse(input: &str) -> Result<String, VfsError> {
        parse_location(input, &at("/srv/data"), Path::new("/home/a")).map(|l| l.display)
    }

    #[test]
    fn absolute_home_relative_and_uri_text_all_parse() {
        assert_eq!(parse("/etc/ssh").unwrap(), "/etc/ssh");
        assert_eq!(parse("  /etc/../usr/ ").unwrap(), "/usr");
        assert_eq!(parse("~").unwrap(), "/home/a");
        assert_eq!(parse("~/Music").unwrap(), "/home/a/Music");
        assert_eq!(parse("logs").unwrap(), "/srv/data/logs");
        assert_eq!(parse("../other").unwrap(), "/srv/other");
        assert_eq!(parse("file:///tmp/x%20y").unwrap(), "/tmp/x y");
        assert_eq!(parse("FILE:///tmp").unwrap(), "/tmp");
    }

    #[test]
    fn the_location_keeps_the_lossless_uri() {
        let location = parse_location("a b", &at("/srv"), Path::new("/h")).unwrap();
        assert_eq!(location.display, "/srv/a b");
        assert_eq!(location.uri, "file:///srv/a%20b");
    }

    #[test]
    fn it_does_not_check_that_the_location_exists() {
        assert_eq!(parse("/no/such/folder").unwrap(), "/no/such/folder");
    }

    #[test]
    fn other_schemes_are_unsupported_and_junk_is_invalid() {
        assert_eq!(
            parse("SFTP://host/x").unwrap_err(),
            VfsError::Unsupported {
                what: "sftp".to_owned()
            }
        );
        for bad in ["", "   ", "a\0b", "file://host/x", "file:///bad%zz"] {
            assert!(
                matches!(parse(bad), Err(VfsError::InvalidLocation { .. })),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_folder_is_described_from_its_root_with_a_parent() {
        let info = describe_location(&at("/home/a/Music")).unwrap();
        let labels: Vec<_> = info.segments.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["/", "home", "a", "Music"]);
        assert_eq!(info.segments[0].location.display, "/");
        assert_eq!(info.segments[2].location.display, "/home/a");
        assert_eq!(info.parent.unwrap().display, "/home/a");
    }

    #[test]
    fn a_root_has_no_parent_and_one_segment() {
        let info = describe_location(&at("/")).unwrap();
        assert_eq!(info.parent, None);
        assert_eq!(info.segments.len(), 1);
        assert_eq!(info.segments[0].label, "/");
    }

    #[test]
    fn a_bad_location_is_invalid() {
        let error = describe_location(&Location::new("x", "nonsense")).unwrap_err();
        assert!(matches!(error, VfsError::InvalidLocation { .. }));
    }
}
