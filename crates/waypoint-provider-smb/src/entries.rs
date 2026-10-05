// The entries both engines make themselves: the server's root and its shares, which are folders
// with no size and no time.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsString;

use waypoint_vfs::{group_for_scan, EntryKind, ScannedEntry};

/// A share, or the server's root, as an entry.
pub(crate) fn folder(name: &str) -> ScannedEntry {
    ScannedEntry {
        name: OsString::from(name),
        kind: EntryKind::Directory,
        link_target: None,
        link_pending: false,
        group: group_for_scan(name.as_bytes(), EntryKind::Directory, None, false, false),
        special: None,
        size: None,
        modified_ms: None,
        hidden: false,
        trashed: None,
        attributes: None,
    }
}
