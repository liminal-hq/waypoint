// Entry details built from what a listing already knows, for providers that cannot read more.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{mime, DetailField, EntryDetails, EntryKind, ScannedEntry};

/// Every detail a provider that only lists and stats cannot report.
pub(crate) const LOCAL_ONLY: [DetailField; 6] = [
    DetailField::AllocatedSize,
    DetailField::Created,
    DetailField::Accessed,
    DetailField::Owner,
    DetailField::Group,
    DetailField::Permissions,
];

/// The portable core of an `EntryDetails`: name, kind, size, modified time, hidden flag and a
/// content type from the name alone. Everything in `LOCAL_ONLY` is listed as unavailable.
pub(crate) fn from_scanned(entry: &ScannedEntry) -> EntryDetails {
    let name = entry.name.to_string_lossy().into_owned();
    let mime_type = if entry.kind == EntryKind::Directory {
        Some("inode/directory".to_owned())
    } else {
        mime::from_name(&name).map(str::to_owned)
    };
    EntryDetails {
        name,
        kind: entry.kind,
        resolves_to: entry.link_target,
        symlink_target: None,
        size: entry.size,
        allocated_size: None,
        created_ms: None,
        modified_ms: entry.modified_ms,
        accessed_ms: None,
        owner: None,
        group: None,
        mode: None,
        read_only: false,
        hidden: entry.hidden,
        mime_type,
        unavailable: LOCAL_ONLY.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;
    use crate::IconGroup;

    #[test]
    fn a_remote_entry_reports_what_it_knows_and_lists_the_rest_as_unavailable() {
        let entry = ScannedEntry {
            name: OsString::from("movie.MP4"),
            kind: EntryKind::File,
            link_target: None,
            link_pending: false,
            group: IconGroup::Video,
            special: None,
            size: Some(42),
            modified_ms: Some(7),
            hidden: false,
            trashed: None,
        };
        let details = from_scanned(&entry);
        assert_eq!(details.size, Some(42));
        assert_eq!(details.modified_ms, Some(7));
        assert_eq!(details.mime_type.as_deref(), Some("video/mp4"));
        assert_eq!(details.unavailable, LOCAL_ONLY.to_vec());
        assert_eq!(details.owner, None);
    }
}
