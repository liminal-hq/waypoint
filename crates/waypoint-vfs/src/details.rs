// The wire types of an entry's details, a folder's total size and a file's text head (A64).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_protocol::VfsError;

use crate::EntryKind;

/// A detail only some providers and platforms can report. An `EntryDetails` lists the ones this
/// provider cannot report in `unavailable`, so a view can say "not available here" instead of
/// showing a blank that looks like "none".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum DetailField {
    /// The space the entry takes on its volume.
    AllocatedSize,
    /// When the entry was created (not every Linux file system records it).
    Created,
    Accessed,
    Owner,
    Group,
    /// The Unix permission bits.
    Permissions,
}

/// Everything the Inspector and the Properties window show about one entry.
///
/// A field that is `None` and not in `unavailable` is a field this entry does not have (a
/// folder's size, a regular file's link target). A field in `unavailable` is one this provider or
/// platform cannot report at all: remote providers report only what a listing already knows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct EntryDetails {
    /// The display name (lossy; identity is the `EntryId`).
    pub name: String,
    pub kind: EntryKind,
    /// For a symlink, what it points at; `None` for a broken link.
    pub resolves_to: Option<EntryKind>,
    /// For a symlink, the text it holds (relative stays relative), lossy.
    pub symlink_target: Option<String>,
    /// The exact size in bytes of a file (a link's target's, when it resolves).
    #[ts(type = "number | null")]
    pub size: Option<u64>,
    /// What the file takes on disk, where that is cheap to learn.
    #[ts(type = "number | null")]
    pub allocated_size: Option<u64>,
    #[ts(type = "number | null")]
    pub created_ms: Option<i64>,
    #[ts(type = "number | null")]
    pub modified_ms: Option<i64>,
    #[ts(type = "number | null")]
    pub accessed_ms: Option<i64>,
    /// The owner's name, or the numeric id when it has no name.
    pub owner: Option<String>,
    pub group: Option<String>,
    /// The Unix permission bits, setuid, setgid and sticky included.
    pub mode: Option<u32>,
    /// Nobody can write to the entry: no write bit is set, or the read-only attribute is.
    pub read_only: bool,
    pub hidden: bool,
    /// The content type, from the name and (for a local file) a sniff of the first bytes. `None`
    /// when neither says.
    pub mime_type: Option<String>,
    pub unavailable: Vec<DetailField>,
}

/// What a recursive folder total has counted so far, or in the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct FolderSizeTotals {
    /// Files counted (anything that is neither a folder nor a symlink), cloud placeholders included.
    #[ts(type = "number")]
    pub files: u64,
    /// Folders below the one asked about.
    #[ts(type = "number")]
    pub folders: u64,
    /// The summed size of the files. A file with several hard links counts once.
    #[ts(type = "number")]
    pub bytes: u64,
    /// What they take on disk, where the platform reports it cheaply (Linux); `None` elsewhere.
    #[ts(type = "number | null")]
    pub allocated_bytes: Option<u64>,
    /// Symlinks passed over (never followed, never counted as size).
    #[ts(type = "number")]
    pub symlinks_skipped: u64,
    /// Folders on another volume passed over (a mount point is never crossed).
    #[ts(type = "number")]
    pub mounts_skipped: u64,
    /// Cloud placeholders (OneDrive and the like) counted as files of zero bytes: reading their
    /// size would not need the data, but descending into or opening them could download it.
    #[ts(type = "number")]
    pub placeholders: u64,
    /// Entries and folders that could not be read (permission denied, gone since the listing).
    #[ts(type = "number")]
    pub unreadable: u64,
}

/// What a folder-size run tells the window that started it. Exactly one of `done`, `cancelled` and
/// `failed` ends the stream, and it carries the totals up to that point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum FolderSizeEvent {
    /// A running total, about every 100 ms while the walk goes on.
    Progress { totals: FolderSizeTotals },
    /// The walk finished; the totals are exact (apart from what `unreadable` counts).
    Done { totals: FolderSizeTotals },
    /// The walk was cancelled; the totals cover what was counted before it stopped.
    Cancelled { totals: FolderSizeTotals },
    /// The folder itself could not be read.
    Failed { error: VfsError },
}

/// The start of a text file, for a preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct TextHead {
    /// The text, decoded as UTF-8 with invalid sequences replaced and any byte-order mark removed.
    pub text: String,
    /// The file continues beyond `text`.
    pub truncated: bool,
    /// Some bytes were not valid UTF-8 and were replaced, so the file may use another encoding.
    pub lossy: bool,
    /// How many bytes of the file `text` covers.
    #[ts(type = "number")]
    pub bytes_read: u64,
}
