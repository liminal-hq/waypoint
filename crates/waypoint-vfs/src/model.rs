// The wire types a listing is read through: entries, sorting, snapshots and live patches.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::special::SpecialFolder;
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

/// Which bundled icon an entry uses. Decided in Rust from the name (and, where the listing has it
/// for free, the execute bit), so the frontend only maps a group to a glyph.
///
/// The first eight are the coarse kinds that sorting, grouping by kind and the kind filter use (see
/// `kind_class`); the rest refine them so an icon set can draw a PDF differently from a text file.
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
    Pdf,
    /// A desktop launcher, an AppImage or a Windows program: something you run to use an app.
    App,
    Text,
    Markdown,
    Spreadsheet,
    Presentation,
    Font,
    DiskImage,
    Database,
    Config,
    ShellScript,
    /// A file with the execute bit that no other group claims, or a binary such as a library.
    Executable,
    /// A key or a certificate.
    Certificate,
    Ebook,
    Torrent,
    Calendar,
    Contact,
    Log,
    Model3d,
    Subtitles,
    Playlist,
    /// A software package or installer (`.deb`, `.rpm`, `.msi`, `.flatpak`).
    Package,
    /// A symbolic link that points at nothing, or at something that is neither a file nor a folder.
    Symlink,
}

impl IconGroup {
    /// Every group, in declaration order.
    pub const ALL: [IconGroup; 31] = [
        IconGroup::Folder,
        IconGroup::Image,
        IconGroup::Audio,
        IconGroup::Video,
        IconGroup::Archive,
        IconGroup::Code,
        IconGroup::Document,
        IconGroup::Other,
        IconGroup::Pdf,
        IconGroup::App,
        IconGroup::Text,
        IconGroup::Markdown,
        IconGroup::Spreadsheet,
        IconGroup::Presentation,
        IconGroup::Font,
        IconGroup::DiskImage,
        IconGroup::Database,
        IconGroup::Config,
        IconGroup::ShellScript,
        IconGroup::Executable,
        IconGroup::Certificate,
        IconGroup::Ebook,
        IconGroup::Torrent,
        IconGroup::Calendar,
        IconGroup::Contact,
        IconGroup::Log,
        IconGroup::Model3d,
        IconGroup::Subtitles,
        IconGroup::Playlist,
        IconGroup::Package,
        IconGroup::Symlink,
    ];

    /// The coarse kind this group belongs to, one of the first eight: what sorting by kind, grouping
    /// by kind and the kind filter see, so refining the icons never reshuffles a listing.
    pub fn kind_class(self) -> IconGroup {
        use IconGroup::*;
        match self {
            Document | Pdf | Text | Markdown | Spreadsheet | Presentation | Ebook | Calendar
            | Contact | Log | Subtitles => Document,
            Code | Config | ShellScript => Code,
            Archive | DiskImage | Package => Archive,
            Audio | Playlist => Audio,
            Folder | Image | Video => self,
            _ => Other,
        }
    }
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
    /// For a folder that is one of the user's own standard folders (Home, Documents, Downloads, …),
    /// which one, so an icon set can mark it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub special: Option<SpecialFolder>,
    /// Size in bytes; `None` for directories and where the size is unknown.
    #[ts(type = "number | null")]
    pub size: Option<u64>,
    /// Modified time in milliseconds since the Unix epoch.
    #[ts(type = "number | null")]
    pub modified_ms: Option<i64>,
    pub hidden: bool,
    /// For an item in the Trash, the folder it was trashed from, as people read it (lossy).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub original_path: Option<String>,
    /// For an item in the Trash, when it was trashed, in milliseconds since the Unix epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null", optional)]
    pub deleted_ms: Option<i64>,
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
    /// When an item was trashed (Trash listings only; elsewhere every entry ties).
    Deleted,
}

/// What a listing is divided into headed groups by. A group is a contiguous run of the sorted rows,
/// so grouping is the first part of the order and the sort orders the rows inside each group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum GroupBy {
    /// One run of rows with no headers.
    #[default]
    None,
    /// The icon group: folders, images, audio, video, archives, code, documents and the rest.
    Kind,
    /// How long ago: today, yesterday, earlier this week, the last 7 and 30 days, this year, then
    /// each earlier year.
    Modified,
    /// A band of file sizes, from empty to gigantic; folders and unknown sizes have a band of their own.
    Size,
    /// The first letter of the name (digits and symbols together).
    Name,
    /// The extension, with folders and names without one ahead of it.
    Type,
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
    /// Divides the sorted rows into groups. Absent in a sort saved before grouping existed, which
    /// reads as no grouping.
    #[serde(default)]
    pub group_by: GroupBy,
}

impl Default for SortSpec {
    fn default() -> Self {
        Self {
            key: SortKey::Name,
            descending: false,
            directories_first: true,
            group_by: GroupBy::None,
        }
    }
}

/// How long ago an entry was modified, as a group of its own. The bands are calendar days in the
/// local time zone, and a week starts on Monday.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ModifiedBucket {
    Today,
    Yesterday,
    EarlierThisWeek,
    Last7Days,
    Last30Days,
    ThisYear,
    /// No modified time is known.
    Unknown,
}

/// A band of file sizes (the thresholds are decimal: 10 kB, 100 kB, 1 MB, 16 MB and 128 MB).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum SizeBand {
    /// Folders and entries whose size is not known.
    Unspecified,
    Empty,
    Tiny,
    Small,
    Medium,
    Large,
    Huge,
    Gigantic,
}

/// What one group is, as a typed key the page turns into a translated heading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum GroupKey {
    /// An icon group (grouping by kind, and the folders ahead of the rest when grouping by type).
    Kind {
        group: IconGroup,
    },
    Modified {
        bucket: ModifiedBucket,
    },
    /// An earlier calendar year than this one.
    Year {
        year: i32,
    },
    Size {
        band: SizeBand,
    },
    /// An upper-case letter, or `#` for a name that starts with a digit or a symbol.
    Name {
        initial: String,
    },
    /// A lower-case extension without its dot; empty for a name without one.
    Type {
        extension: String,
    },
}

/// One group of the view: the contiguous run of `count` rows from view position `start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct GroupRun {
    pub key: GroupKey,
    pub start: u32,
    pub count: u32,
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

/// Which columns and actions a listing's entries make sense with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ListingLayout {
    /// Ordinary folder contents: size, modified time and kind.
    #[default]
    Folder,
    /// The Trash: the original location and the deletion date, items that cannot be opened or
    /// changed in place.
    Trash,
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
    /// The provider writes nothing here, so the view offers no new, rename, paste or drop.
    pub read_only: bool,
    pub layout: ListingLayout,
    /// The groups of the view in order, covering every row; empty when the sort does not group.
    pub groups: Vec<GroupRun>,
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
        /// The groups of the view when a scan has just filled it (and the sort groups); absent
        /// otherwise, where they have not changed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        groups: Option<Vec<GroupRun>>,
    },
    /// The view changed; apply the operations in order to any cached pages.
    Changed {
        handle: ListingHandle,
        revision: u32,
        count: u32,
        ops: Vec<PatchOp>,
        /// Entries the `ops` removed and inserted again because their place in the order changed
        /// (a rename, or a new size under a size sort). They keep their `EntryId`, so a consumer
        /// keeps what it holds by id (a selection) instead of forgetting it with the removal.
        moved: Vec<EntryId>,
        /// The groups of the view after the `ops`, whole, when the sort groups: a row that changed
        /// group moved with its header, so boundaries are sent again rather than patched.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        groups: Option<Vec<GroupRun>>,
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
    /// The login the location belongs to (`sftp://me@nas.lan`), for a server location or an
    /// archive on one; absent for a local folder. The tab's remote badge and state follow it, so
    /// the page never reads a scheme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub connection: Option<String>,
}

/// Typed text read into a location, and whether a password written in it was dropped (D147).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct TypedLocation {
    pub location: Location,
    pub password_dropped: bool,
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

/// What a destination picker needs to know about a location before it offers it: it is there (a
/// missing one is rejected with `NotFound` instead), and whether it is a folder that can be written
/// to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct FolderCheck {
    /// A folder, or a link to one.
    pub is_folder: bool,
    /// Something can be written there: its provider writes at all and the folder's own permissions
    /// allow it. Where the permissions cannot be read this is `true`, and a refusal shows when the
    /// job runs.
    pub writable: bool,
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
        assert_eq!(sort.group_by, GroupBy::None);
    }

    #[test]
    fn a_sort_saved_before_grouping_reads_as_ungrouped() {
        let sort: SortSpec =
            serde_json::from_str(r#"{"key":"size","descending":true,"directoriesFirst":false}"#)
                .unwrap();
        assert_eq!(sort.key, SortKey::Size);
        assert_eq!(sort.group_by, GroupBy::None);
    }

    #[test]
    fn group_keys_are_tagged_with_their_value() {
        let json = serde_json::to_string(&GroupKey::Modified {
            bucket: ModifiedBucket::EarlierThisWeek,
        })
        .unwrap();
        assert_eq!(json, r#"{"kind":"modified","bucket":"earlierThisWeek"}"#);
    }

    #[test]
    fn events_carry_a_kind_tag_and_camel_case_fields() {
        let json = serde_json::to_string(&ListingEvent::Progress {
            handle: ListingHandle(3),
            revision: 2,
            phase: ListingPhase::Scanning,
            scanned: 10,
            count: 10,
            groups: None,
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
