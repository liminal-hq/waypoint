// From the properties of a `response` to the entry a listing shows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::time::UNIX_EPOCH;

use waypoint_vfs::{group_for_scan, EntryKind, ScannedEntry};

use crate::paths::os_name;
use crate::xml::Props;

/// The modification time in milliseconds since the epoch, from any of the three date formats
/// HTTP allows. A date a server writes wrongly is no date at all.
pub(crate) fn modified_ms(text: &str) -> Option<i64> {
    let time = httpdate::parse_http_date(text.trim()).ok()?;
    let since = time.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(since.as_millis()).ok()
}

/// An entry named `name`. A server that leaves `resourcetype` out is taken at its `href`: a
/// trailing slash (`folder_hint`) means a collection. A folder has no size here (a collection's
/// `getcontentlength` is not the size of what it holds).
pub(crate) fn entry(name: &[u8], props: &Props, folder_hint: bool) -> ScannedEntry {
    let kind = if props.collection.unwrap_or(folder_hint) {
        EntryKind::Directory
    } else {
        EntryKind::File
    };
    ScannedEntry {
        name: os_name(name),
        kind,
        link_target: None,
        link_pending: false,
        group: group_for_scan(name, kind, None, false, false),
        special: None,
        size: props.length.filter(|_| kind == EntryKind::File),
        modified_ms: props.modified.as_deref().and_then(modified_ms),
        hidden: name.starts_with(b"."),
        trashed: None,
        attributes: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_in_every_http_format_read() {
        let wanted = Some(784_111_777_000);
        assert_eq!(modified_ms("Sun, 06 Nov 1994 08:49:37 GMT"), wanted);
        assert_eq!(modified_ms("Sunday, 06-Nov-94 08:49:37 GMT"), wanted);
        assert_eq!(modified_ms("Sun Nov  6 08:49:37 1994"), wanted);
        assert_eq!(modified_ms("yesterday"), None);
        assert_eq!(modified_ms(""), None);
    }

    #[test]
    fn a_server_that_omits_properties_still_gives_an_entry() {
        let bare = Props::default();
        let file = entry(b"a.txt", &bare, false);
        assert_eq!(file.kind, EntryKind::File);
        assert_eq!((file.size, file.modified_ms), (None, None));
        assert_eq!(entry(b"sub", &bare, true).kind, EntryKind::Directory);
        assert!(entry(b".hidden", &bare, false).hidden);
        let declared = Props {
            collection: Some(true),
            length: Some(4096),
            ..Props::default()
        };
        let folder = entry(b"sub", &declared, false);
        assert_eq!(folder.kind, EntryKind::Directory);
        assert_eq!(folder.size, None);
    }
}
