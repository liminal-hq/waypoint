// Shelf commands: add, remove, clear and reorder the references every window shares.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashSet;

use waypoint_protocol::Location;

use crate::model::{folder_name, ShelfItem, ShelfItemId};
use crate::reducer::{Command, SessionError};
use crate::store::{Store, SHELF_LIMIT};

/// The folder a location sits in, which is what the Shelf groups by. A root has no parent and is
/// its own origin. Works on the text of the URI and the display path, as the rest of this crate
/// does, so it needs no filesystem and no scheme knowledge beyond `/` and `\` separators.
pub(crate) fn origin_of(location: &Location) -> Location {
    let uri = parent_uri(&location.uri);
    if uri == location.uri {
        return location.clone();
    }
    Location::new(parent_display(&location.display), uri)
}

/// Whether `path` (the part of a URI after its authority) is only a Windows drive, `/C:`.
fn is_drive(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() == 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':'
}

/// The URI one level up, or `uri` itself for a root (and for text that is not a URI).
fn parent_uri(uri: &str) -> String {
    let Some(scheme_end) = uri.find("://") else {
        return uri.to_string();
    };
    let path_start = scheme_end + 3;
    let trimmed = uri.trim_end_matches('/');
    if trimmed.len() <= path_start || is_drive(&trimmed[path_start..]) {
        return uri.to_string();
    }
    // A slash at `path_start` is the root path of an empty authority (`file:///a`).
    let Some(slash) = trimmed.rfind('/').filter(|i| *i >= path_start) else {
        return uri.to_string();
    };
    if slash == path_start {
        return uri[..=slash].to_string();
    }
    let parent = &trimmed[..slash];
    if is_drive(&parent[path_start..]) {
        format!("{parent}/")
    } else {
        parent.to_string()
    }
}

fn parent_display(display: &str) -> String {
    let trimmed = display.trim_end_matches(['/', '\\']);
    match trimmed.rfind(['/', '\\']) {
        None => display.to_string(),
        Some(0) => display[..1].to_string(),
        Some(i) => {
            let parent = &trimmed[..i];
            if parent.ends_with(':') {
                format!("{parent}{}", &trimmed[i..=i])
            } else {
                parent.to_string()
            }
        }
    }
}

fn item(id: u64, location: Location, added_ms: u64) -> ShelfItem {
    ShelfItem {
        id: ShelfItemId(id),
        name: folder_name(&location.display).to_string(),
        origin: origin_of(&location),
        location,
        added_ms,
    }
}

pub(crate) fn apply(store: &mut Store, window: &str, command: Command) -> Result<(), SessionError> {
    // The Shelf is global but its events go to windows, so a command needs a live caller:
    // otherwise a change could happen with nobody told.
    store.window_index(window)?;
    match command {
        Command::AddToShelf {
            locations,
            added_ms,
        } => {
            let mut seen: HashSet<String> =
                store.shelf.iter().map(|i| i.location.uri.clone()).collect();
            let fresh: Vec<Location> = locations
                .into_iter()
                .filter(|l| seen.insert(l.uri.clone()))
                .collect();
            if store.shelf.len() + fresh.len() > SHELF_LIMIT {
                return Err(SessionError::ShelfFull(SHELF_LIMIT));
            }
            for location in fresh {
                let id = store.next_shelf;
                store.next_shelf += 1;
                store.shelf.push(item(id, location, added_ms));
            }
        }
        Command::RemoveFromShelf { ids } => {
            let gone: HashSet<ShelfItemId> = ids.into_iter().collect();
            store.shelf.retain(|i| !gone.contains(&i.id));
        }
        Command::ClearShelf => store.shelf.clear(),
        Command::MoveShelfItem { id, to_index } => {
            let at = store
                .shelf
                .iter()
                .position(|i| i.id == id)
                .ok_or(SessionError::UnknownShelfItem(id.0))?;
            let moved = store.shelf.remove(at);
            store.shelf.insert(to_index.min(store.shelf.len()), moved);
        }
        _ => unreachable!("only Shelf commands reach the Shelf module"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(display: &str, uri: &str) -> Location {
        Location::new(display, uri)
    }

    #[test]
    fn the_origin_is_the_containing_folder() {
        let o = origin_of(&loc("/home/a/b.txt", "file:///home/a/b.txt"));
        assert_eq!(
            (o.display.as_str(), o.uri.as_str()),
            ("/home/a", "file:///home/a")
        );
        let o = origin_of(&loc("/b.txt", "file:///b.txt"));
        assert_eq!((o.display.as_str(), o.uri.as_str()), ("/", "file:///"));
        let o = origin_of(&loc("/home/a/", "file:///home/a/"));
        assert_eq!(
            (o.display.as_str(), o.uri.as_str()),
            ("/home", "file:///home")
        );
    }

    #[test]
    fn a_root_is_its_own_origin() {
        let r = loc("/", "file:///");
        assert_eq!(origin_of(&r), r);
    }

    #[test]
    fn windows_paths_have_drive_roots() {
        let o = origin_of(&loc("C:\\Users\\a.txt", "file:///C:/Users/a.txt"));
        assert_eq!(
            (o.display.as_str(), o.uri.as_str()),
            ("C:\\Users", "file:///C:/Users")
        );
        let o = origin_of(&loc("C:\\a.txt", "file:///C:/a.txt"));
        assert_eq!(
            (o.display.as_str(), o.uri.as_str()),
            ("C:\\", "file:///C:/")
        );
        let r = loc("C:\\", "file:///C:/");
        assert_eq!(origin_of(&r), r);
    }

    #[test]
    fn a_remote_location_groups_under_its_folder() {
        let o = origin_of(&loc("host/dir/f", "sftp://host/dir/f"));
        assert_eq!(o.uri, "sftp://host/dir");
        let r = loc("host", "sftp://host");
        assert_eq!(origin_of(&r), r);
    }
}
