// Turning typed text into locations, and locations into breadcrumbs: the path logic of the path bar
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;

use waypoint_path::{
    ArchivePath, FilePath, PathError, RemoteScheme, TrashPath, VfsPath, ARCHIVE_SCHEME,
};
use waypoint_protocol::{Location, VfsError};

use crate::model::{Breadcrumb, LocationInfo};
use crate::places::{is_overview_uri, overview_location};

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
    parse_location_with(input, base, home, &|_| false).map(|(location, _)| location)
}

/// `parse_location`, also reading the server, archive and Git schemes for which `serves` is true
/// (the schemes of the registered providers, A78), so text in a scheme nothing serves is still
/// `Unsupported`. The second value says whether a password written in the text was dropped: the
/// location never holds one, and the path bar says it was not kept (D147).
pub fn parse_location_with(
    input: &str,
    base: &Location,
    home: &Path,
    serves: &dyn Fn(&str) -> bool,
) -> Result<(Location, bool), VfsError> {
    let text = input.trim();
    if text.is_empty() || text.contains('\0') {
        return Err(invalid(input));
    }
    // An archive location has no `//` either.
    if ArchivePath::is_archive_uri(text) {
        return parse_served(input, text, ARCHIVE_SCHEME, serves);
    }
    if let Some(scheme) = scheme_of(text) {
        if !scheme.eq_ignore_ascii_case("file") {
            return parse_served(input, text, scheme, serves);
        }
    }
    // Text relative to a server, archive or revision folder stays there.
    let relative = !(text.starts_with('/')
        || text.starts_with('~')
        || (cfg!(windows) && (text.starts_with('\\') || text.get(1..2) == Some(":"))));
    if relative {
        if let Ok(base @ (VfsPath::Remote(_) | VfsPath::Archive(_) | VfsPath::Git(_))) =
            VfsPath::from_location(base)
        {
            if serves(base.scheme()) {
                return base
                    .join(text)
                    .map(|path| (path.to_location(), false))
                    .map_err(|_| invalid(input));
            }
        }
    }
    parse_local(input, text, base, home).map(|location| (location, false))
}

/// Parses text in a scheme other than `file`, when a provider serves it.
fn parse_served(
    input: &str,
    text: &str,
    scheme: &str,
    serves: &dyn Fn(&str) -> bool,
) -> Result<(Location, bool), VfsError> {
    let scheme = RemoteScheme::from_name(scheme)
        .map(|scheme| scheme.as_str().to_owned())
        .unwrap_or_else(|| scheme.to_ascii_lowercase());
    if !serves(&scheme) {
        return Err(VfsError::Unsupported { what: scheme });
    }
    VfsPath::parse_input_reporting(text)
        .map(|(path, dropped)| (path.to_location(), dropped))
        .map_err(|_| invalid(input))
}

fn parse_local(
    input: &str,
    text: &str,
    base: &Location,
    home: &Path,
) -> Result<Location, VfsError> {
    // Overview, like `trash:/`, has no `//`.
    if is_overview_uri(text) {
        return Ok(overview_location());
    }
    // `trash:/` has no `//`, so it is not a scheme `scheme_of` sees.
    if TrashPath::is_trash_uri(text) {
        return VfsPath::from_uri(text)
            .map(|path| path.to_location())
            .map_err(|_| invalid(input));
    }
    if scheme_of(text).is_some() {
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
    // Overview is one level: a view with no parent, whose only segment is itself.
    if is_overview_uri(&location.uri) {
        return Ok(LocationInfo {
            parent: None,
            segments: vec![Breadcrumb {
                label: "Overview".to_owned(),
                location: overview_location(),
            }],
            connection: None,
        });
    }
    if TrashPath::is_trash_uri(&location.uri) {
        return describe_trash(location);
    }
    if let Ok(path @ (VfsPath::Remote(_) | VfsPath::Archive(_) | VfsPath::Git(_))) =
        VfsPath::from_location(location)
    {
        return Ok(describe_chain(path));
    }
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
    Ok(LocationInfo {
        parent,
        segments,
        connection: None,
    })
}

/// A server, archive or revision location, from the root of its chain of parents: a server's root
/// is labelled with its login, and an archive's or a revision's breadcrumbs continue from the
/// folder that holds it.
fn describe_chain(path: VfsPath) -> LocationInfo {
    let connection = path.connection_key().map(|key| key.as_str().to_owned());
    let mut segments = Vec::new();
    let mut current = Some(path);
    while let Some(here) = current {
        let next = here.parent();
        segments.push(Breadcrumb {
            label: here.label(),
            location: here.to_location(),
        });
        current = next;
    }
    segments.reverse();
    let parent = match segments.len() {
        0 | 1 => None,
        n => Some(segments[n - 2].location.clone()),
    };
    LocationInfo {
        parent,
        segments,
        connection,
    }
}

/// The Trash is one level: its root, and below that the items.
fn describe_trash(location: &Location) -> Result<LocationInfo, VfsError> {
    let path = TrashPath::from_uri(&location.uri).map_err(|_| invalid(&location.uri))?;
    let root = Breadcrumb {
        label: TrashPath::Root.display(),
        location: VfsPath::Trash(TrashPath::Root).to_location(),
    };
    match path {
        TrashPath::Root => Ok(LocationInfo {
            parent: None,
            segments: vec![root],
            connection: None,
        }),
        TrashPath::Item(id) => Ok(LocationInfo {
            parent: Some(root.location.clone()),
            segments: vec![
                root,
                Breadcrumb {
                    label: id.clone(),
                    location: VfsPath::Trash(TrashPath::Item(id)).to_location(),
                },
            ],
            connection: None,
        }),
    }
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
    fn the_trash_parses_and_is_described_from_its_root() {
        for text in ["trash:/", "trash:", " TRASH:/ "] {
            let location = parse_location(text, &at("/srv"), Path::new("/h")).unwrap();
            assert_eq!(location, Location::new("Trash", "trash:/"), "{text}");
        }
        let root = describe_location(&Location::new("Trash", "trash:/")).unwrap();
        assert_eq!(root.parent, None);
        assert_eq!(root.segments.len(), 1);
        assert_eq!(root.segments[0].label, "Trash");
        let item = describe_location(&Location::new("x", "trash:/a%7Cb")).unwrap();
        assert_eq!(item.parent.unwrap().uri, "trash:/");
        assert_eq!(item.segments[1].label, "a|b");
        assert!(describe_location(&Location::new("x", "trash:/a/b")).is_err());
    }

    #[test]
    fn overview_parses_and_is_described_as_a_single_view() {
        for text in ["overview:/", "overview:", " OVERVIEW:/ "] {
            let location = parse_location(text, &at("/srv"), Path::new("/h")).unwrap();
            assert_eq!(location, Location::new("Overview", "overview:/"), "{text}");
        }
        let info = describe_location(&Location::new("Overview", "overview:/")).unwrap();
        assert_eq!(info.parent, None);
        assert_eq!(info.segments.len(), 1);
        assert_eq!(info.segments[0].label, "Overview");
    }

    fn serves_everything(_: &str) -> bool {
        true
    }

    #[test]
    fn served_schemes_parse_and_others_stay_unsupported() {
        let base = at("/srv");
        let home = Path::new("/h");
        let parse = |text: &str| parse_location_with(text, &base, home, &serves_everything);
        let (location, dropped) = parse("SFTP://Me:pw@NAS:22/srv/a b").unwrap();
        assert!(dropped);
        assert_eq!(location.uri, "sftp://Me@nas/srv/a%20b");
        assert_eq!(location.display, "sftp://Me@nas/srv/a b");
        let (location, _) = parse("webdavs://cloud/x").unwrap();
        assert_eq!(location.uri, "davs://cloud/x");
        let (location, _) = parse("archive:/tmp/a.zip").unwrap();
        assert_eq!(location.uri, "archive:file:///tmp/a.zip!/");
        // Local text is unchanged by the extra schemes.
        assert_eq!(parse("logs").unwrap().0.display, "/srv/logs");
        // Text relative to a server folder stays on the server.
        let remote = Location::new("x", "sftp://h/srv");
        let (location, _) =
            parse_location_with("../logs", &remote, home, &serves_everything).unwrap();
        assert_eq!(location.uri, "sftp://h/logs");
        // Nothing serves `sftp` here: the old answer.
        let only_files = |scheme: &str| scheme == "file";
        assert_eq!(
            parse_location_with("webdav://h/x", &base, home, &only_files).unwrap_err(),
            VfsError::Unsupported {
                what: "dav".to_owned()
            }
        );
        assert_eq!(
            parse_location_with("archive:/tmp/a.zip", &base, home, &only_files).unwrap_err(),
            VfsError::Unsupported {
                what: "archive".to_owned()
            }
        );
        assert!(matches!(
            parse("sftp://"),
            Err(VfsError::InvalidLocation { .. })
        ));
    }

    #[test]
    fn a_server_is_described_from_its_login() {
        let info = describe_location(&Location::new("x", "sftp://me@nas:2222/srv/media")).unwrap();
        let labels: Vec<_> = info.segments.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["me@nas:2222", "srv", "media"]);
        assert_eq!(info.parent.unwrap().uri, "sftp://me@nas:2222/srv");
        assert_eq!(info.connection.as_deref(), Some("sftp://me@nas:2222"));
        let local = describe_location(&at("/srv")).unwrap();
        assert_eq!(local.connection, None);
        let root = describe_location(&Location::new("x", "smb://files/")).unwrap();
        assert_eq!(root.parent, None);
        assert_eq!(root.segments[0].label, "files");
    }

    #[test]
    fn an_archive_continues_the_breadcrumbs_of_its_folder() {
        let info = describe_location(&Location::new("x", "archive:file:///home/a.zip!/b")).unwrap();
        let labels: Vec<_> = info.segments.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["/", "home", "a.zip", "b"]);
        assert_eq!(
            info.segments[2].location.uri,
            "archive:file:///home/a.zip!/"
        );
        assert_eq!(info.segments[1].location.uri, "file:///home");
        let revision = describe_location(&Location::new("x", "git+file:///r!/src?rev=v1")).unwrap();
        let labels: Vec<_> = revision.segments.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["/", "r", "r @ v1", "src"]);
    }

    #[test]
    fn a_bad_location_is_invalid() {
        let error = describe_location(&Location::new("x", "nonsense")).unwrap_err();
        assert!(matches!(error, VfsError::InvalidLocation { .. }));
    }
}
