// The path of a location in the Trash: the Trash itself, or one trashed item named by its receipt id.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::encoding::{decode, encode_into};
use crate::PathError;

/// The URI scheme of the Trash.
pub const TRASH_SCHEME: &str = "trash";

/// Where something is in the Trash. The Trash is flat: a `trash:/` root holds every trashed item,
/// whichever volume or folder it came from, and an item is named by the id the Trash gave it when
/// it took the item (its receipt id), percent-encoded, so any id survives the round trip. The
/// contents of a trashed folder are not addressable through the view.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TrashPath {
    /// `trash:/`.
    Root,
    /// `trash:/{id}`.
    Item(String),
}

impl TrashPath {
    /// Whether `uri` is written in the Trash's scheme, whatever follows.
    pub fn is_trash_uri(uri: &str) -> bool {
        uri.get(..TRASH_SCHEME.len() + 1).is_some_and(|head| {
            head[..TRASH_SCHEME.len()].eq_ignore_ascii_case(TRASH_SCHEME) && head.ends_with(':')
        })
    }

    /// The lossless URI: `trash:/` for the root and `trash:/` plus the percent-encoded id for an
    /// item.
    pub fn to_uri(&self) -> String {
        match self {
            TrashPath::Root => format!("{TRASH_SCHEME}:/"),
            TrashPath::Item(id) => {
                let mut out = format!("{TRASH_SCHEME}:/");
                encode_into(&mut out, id.as_bytes(), &[]);
                out
            }
        }
    }

    /// Reads a Trash URI. `trash:`, `trash:/` and `trash:///` all name the root; one more
    /// segment names an item. An id that is not valid UTF-8 once decoded, or a path of more than
    /// one segment, is not a Trash path.
    pub fn from_uri(uri: &str) -> Result<Self, PathError> {
        if !Self::is_trash_uri(uri) {
            let scheme = uri.split(':').next().unwrap_or_default().to_owned();
            return Err(PathError::UnsupportedScheme(scheme));
        }
        let mut rest = &uri[TRASH_SCHEME.len() + 1..];
        // An empty authority (`trash:///x`) is the same as none.
        if let Some(after) = rest.strip_prefix("//") {
            if !(after.is_empty() || after.starts_with('/')) {
                return Err(PathError::InvalidUri("a trash URI has no host"));
            }
            rest = after;
        }
        let segment = match rest.strip_prefix('/') {
            None if rest.is_empty() => return Ok(TrashPath::Root),
            None => return Err(PathError::InvalidUri("a trash URI path starts with `/`")),
            Some(segment) => segment.strip_suffix('/').unwrap_or(segment),
        };
        if segment.is_empty() {
            return Ok(TrashPath::Root);
        }
        if segment.contains('/') {
            return Err(PathError::InvalidUri("a trashed item has no path below it"));
        }
        let bytes = decode(segment)?;
        let id = String::from_utf8(bytes)
            .map_err(|_| PathError::InvalidUri("a trashed item's id is not text"))?;
        if id.contains('\0') {
            return Err(PathError::InteriorNul);
        }
        Ok(TrashPath::Item(id))
    }

    /// What people read: `Trash`, or `Trash/` and the id for an item (the item's own name is the
    /// listing's to show).
    pub fn display(&self) -> String {
        match self {
            TrashPath::Root => "Trash".to_owned(),
            TrashPath::Item(id) => format!("Trash/{id}"),
        }
    }

    /// The item inside this path: only the root has children.
    pub fn join(&self, child: &str) -> Result<Self, PathError> {
        match self {
            TrashPath::Root if child.is_empty() || child.contains('\0') => {
                Err(PathError::Invalid("an item id is not empty or NUL"))
            }
            TrashPath::Root => Ok(TrashPath::Item(child.to_owned())),
            TrashPath::Item(_) => Err(PathError::Invalid("a trashed item has no children")),
        }
    }

    /// The Trash for an item; nothing for the Trash itself.
    pub fn parent(&self) -> Option<Self> {
        match self {
            TrashPath::Root => None,
            TrashPath::Item(_) => Some(TrashPath::Root),
        }
    }

    /// The item's id; `None` for the root.
    pub fn id(&self) -> Option<&str> {
        match self {
            TrashPath::Root => None,
            TrashPath::Item(id) => Some(id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_is_trash_slash_in_every_spelling() {
        for text in ["trash:", "trash:/", "trash:///", "TRASH:/", "trash://"] {
            assert_eq!(TrashPath::from_uri(text), Ok(TrashPath::Root), "{text}");
        }
        assert_eq!(TrashPath::Root.to_uri(), "trash:/");
    }

    #[test]
    fn an_id_with_awkward_characters_survives_the_round_trip() {
        for id in [
            "%2Fhome%2Fa%2F.local%2Fshare%2FTrash|a b.txt",
            "/tmp/.Trash-1000|100%.txt",
            "x|é#?&=+ \u{1F600}",
            "|",
            "a.b-c_d~e",
        ] {
            let path = TrashPath::Item(id.to_owned());
            let uri = path.to_uri();
            assert!(!uri[7..].contains(['/', ' ', '|', '#', '?']), "{uri}");
            assert_eq!(TrashPath::from_uri(&uri), Ok(path));
        }
    }

    #[test]
    fn an_item_has_the_root_as_its_parent_and_the_root_has_none() {
        let item = TrashPath::Root.join("a|b").unwrap();
        assert_eq!(item.parent(), Some(TrashPath::Root));
        assert_eq!(item.id(), Some("a|b"));
        assert_eq!(TrashPath::Root.parent(), None);
        assert!(item.join("c").is_err());
        // A slash in an id is part of the id, and travels percent-encoded.
        let slashed = TrashPath::Root.join("a/b").unwrap();
        assert_eq!(slashed.to_uri(), "trash:/a%2Fb");
        assert_eq!(TrashPath::from_uri(&slashed.to_uri()), Ok(slashed));
        assert!(TrashPath::Root.join("").is_err());
    }

    #[test]
    fn nonsense_is_refused() {
        for text in [
            "trash:/a/b",
            "trash://host/x",
            "trash:x",
            "trash:/%FF",
            "trash:/%zz",
        ] {
            assert!(TrashPath::from_uri(text).is_err(), "{text}");
        }
        assert_eq!(
            TrashPath::from_uri("sftp://h/a"),
            Err(PathError::UnsupportedScheme("sftp".to_owned()))
        );
    }

    #[test]
    fn it_reads_as_a_trash_uri_only_by_scheme() {
        assert!(TrashPath::is_trash_uri("trash:/x"));
        assert!(TrashPath::is_trash_uri("Trash:"));
        assert!(!TrashPath::is_trash_uri("trashy:/"));
        assert!(!TrashPath::is_trash_uri("file:///trash:"));
        assert!(!TrashPath::is_trash_uri("tra"));
    }

    #[test]
    fn people_read_trash() {
        assert_eq!(TrashPath::Root.display(), "Trash");
        assert_eq!(TrashPath::Item("a".into()).display(), "Trash/a");
    }
}
