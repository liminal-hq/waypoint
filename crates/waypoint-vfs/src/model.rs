// The wire types a listing is read through: entries, sorting, snapshots and live patches.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_protocol::{EntryId, Location, VfsError};

/// What an entry is on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
    Other,
}

/// Which bundled icon an entry uses. Decided in Rust from the extension, so the frontend only maps
/// a group to a glyph (milestone 5 replaces these with theme lookup and thumbnails).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum IconGroup {
    Folder,
    Image,
    Audio,
    Video,
    Archive,
    Code,
    Document,
    Other,
}

/// One row of a listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Entry {
    pub id: EntryId,
    /// The display name. It may be lossy; identity is `id`, never this string.
    pub name: String,
    pub kind: EntryKind,
    /// For a symlink, what it points at, resolved when the listing was scanned (so
    /// directories-first sorting is correct).
    pub link_target: Option<EntryKind>,
    pub group: IconGroup,
    /// Size in bytes; `None` for directories and where the size is unknown.
    #[ts(type = "number | null")]
    pub size: Option<u64>,
    /// Modified time in milliseconds since the Unix epoch.
    #[ts(type = "number | null")]
    pub modified_ms: Option<i64>,
    pub hidden: bool,
}

/// The column a listing is sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum SortKey {
    Name,
    Size,
    Modified,
    Kind,
}

/// How a listing is ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SortSpec {
    pub key: SortKey,
    pub descending: bool,
    /// Folders sort before files whatever the key (a setting, on by default).
    pub directories_first: bool,
}

impl Default for SortSpec {
    fn default() -> Self {
        Self {
            key: SortKey::Name,
            descending: false,
            directories_first: true,
        }
    }
}

/// Restricts a listing to one kind of entry. A symlink to a folder counts as a folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum KindFilter {
    Directories,
    Files,
}

/// What a listing leaves out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Filter {
    pub show_hidden: bool,
    /// Keep only folders (the Folders tree) or only files; everything when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub only: Option<KindFilter>,
}

/// Names one open listing. Handles belong to the window that opened them and close with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ListingHandle(pub u32);

/// Where a listing is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ListingPhase {
    /// The first scan is running; `count` grows as it goes.
    Scanning,
    /// Complete and being kept up to date.
    Ready,
    /// A rescan is running after the watcher lost events or was overwhelmed.
    Rescanning,
    /// The listing cannot continue; see the matching `ListingEvent::Failed`.
    Failed,
}

/// The state of a listing at one revision. Returned when a listing opens and whenever the
/// sort or filter changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ListingSnapshot {
    pub handle: ListingHandle,
    pub location: Location,
    /// Increases with every change, so a late reply can never overwrite a newer state.
    pub revision: u32,
    /// Entries in the current view, after filtering.
    pub count: u32,
    pub phase: ListingPhase,
    pub sort: SortSpec,
    pub filter: Filter,
}

/// One edit to the current view, in view positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum PatchOp {
    /// `count` new entries now sit at `at`.
    Insert { at: u32, count: u32 },
    /// `count` entries starting at `at` are gone.
    Remove { at: u32, count: u32 },
    /// `count` entries starting at `at` changed in place.
    Update { at: u32, count: u32 },
    /// Everything changed; drop every cached page.
    Reset,
}

/// Something that happened to an open listing after it opened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ListingEvent {
    /// Scan progress and phase changes. It carries counts only, never the entries: the listing
    /// owns the rows and the frontend fetches ranges (A9, A18).
    Progress {
        handle: ListingHandle,
        revision: u32,
        phase: ListingPhase,
        scanned: u32,
        count: u32,
    },
    /// The view changed; apply the operations in order to any cached pages.
    Changed {
        handle: ListingHandle,
        revision: u32,
        count: u32,
        ops: Vec<PatchOp>,
    },
    /// The listing cannot continue.
    Failed {
        handle: ListingHandle,
        error: VfsError,
    },
}

/// One step of the path to a location, for the path bar's breadcrumbs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct Breadcrumb {
    /// The segment's display name (a volume root reads as its own name, for example `/`).
    pub label: String,
    /// Where choosing this segment goes.
    pub location: Location,
}

/// What the path bar and the Up button need to know about a location, decided in Rust so the
/// frontend never splits or joins a path itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct LocationInfo {
    /// The containing folder; `None` at a root, where Up is disabled.
    pub parent: Option<Location>,
    /// From the root to the location itself, the last being the location.
    pub segments: Vec<Breadcrumb>,
}

/// Which entries of a listing are selected. The selection is a frontend model keyed by `EntryId`
/// and may be "everything except these ids" over half a million rows, so it crosses the wire in
/// that shape instead of being expanded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum SelectionSpec {
    /// Exactly these entries.
    #[serde(rename = "some")]
    Chosen { ids: Vec<EntryId> },
    /// Every entry in the current view except these.
    #[serde(rename = "allExcept")]
    AllExcept { ids: Vec<EntryId> },
}

/// What a selection adds up to, worked out where the listing lives so the frontend never has to
/// hold or fetch the selected entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct SelectionSummary {
    /// Entries of the current view the selection covers (ids the view no longer holds do not count).
    #[ts(type = "number")]
    pub count: u64,
    /// The summed size in bytes of the selected files. Folders and entries of unknown size add
    /// nothing; it is not a recursive total.
    #[ts(type = "number")]
    pub total_size: u64,
}

/// Space on the volume that holds a location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct VolumeSpace {
    #[ts(type = "number")]
    pub free_bytes: u64,
    #[ts(type = "number")]
    pub total_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_sort_by_name_with_folders_first_and_hide_hidden_files() {
        let sort = SortSpec::default();
        assert_eq!(sort.key, SortKey::Name);
        assert!(!sort.descending);
        assert!(sort.directories_first);
        assert!(!Filter::default().show_hidden);
    }

    #[test]
    fn events_carry_a_kind_tag_and_camel_case_fields() {
        let json = serde_json::to_string(&ListingEvent::Progress {
            handle: ListingHandle(3),
            revision: 2,
            phase: ListingPhase::Scanning,
            scanned: 10,
            count: 10,
        })
        .unwrap();
        assert_eq!(
            json,
            r#"{"kind":"progress","handle":3,"revision":2,"phase":"scanning","scanned":10,"count":10}"#
        );
    }

    #[test]
    fn patch_operations_are_tagged() {
        let json = serde_json::to_string(&PatchOp::Insert { at: 4, count: 2 }).unwrap();
        assert_eq!(json, r#"{"kind":"insert","at":4,"count":2}"#);
        assert_eq!(
            serde_json::to_string(&PatchOp::Reset).unwrap(),
            r#"{"kind":"reset"}"#
        );
    }
}
