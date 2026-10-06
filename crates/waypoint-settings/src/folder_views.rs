// What each folder remembers about how it is shown: its view, sort and grouping, bounded and pruned
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A folder that has had its view, sort, grouping, hidden-files choice, icon size or list column widths changed keeps that choice (SPEC 5.3b). This is
// the pure store behind it: a map from a folder's location to the choices made there, in the order
// they were last written (oldest first), so that going over `MAX_FOLDERS` drops the folder
// written longest ago. Only what was chosen is kept: a folder whose sort was changed and whose
// view never was has no view of its own and shows the window's. The store has one writer (the
// plugin holds it behind a lock), a revision that grows by one per change, and each change says
// exactly which folders it touched, so a window can follow along without reading the whole map.
//
// The map is saved as its own document, with one earlier generation like the settings document,
// and is one of the configuration files the settings export carries.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use ts_rs::TS;
use waypoint_session::ViewMode;
use waypoint_vfs::SortSpec;

use crate::bundle::BundleError;
use crate::import::{unknown_paths, ChangeGroup, FilePlan, ImportWarning};
use crate::storage::{KeyValue, StorageError};

/// The most folders that are remembered. A person who changes the view of more folders than this
/// loses the ones changed longest ago; the cap keeps the file, the export and every window's copy
/// small (about 250 KB at the cap).
pub const MAX_FOLDERS: usize = 1000;

/// The longest location that is remembered, in bytes. A longer one is refused, not cut.
pub const MAX_KEY_BYTES: usize = 4096;

/// The smallest and largest grid icon size a folder remembers, in pixels (the grid's own range).
pub const ICON_SIZE_MIN: u32 = 48;
pub const ICON_SIZE_MAX: u32 = 256;

/// The shortest and longest width a list column may be given, in pixels. The list keeps each
/// column to its own narrower range (so its heading still fits); these bounds only stop a
/// hand-edited document from asking for a column that cannot be seen or one wider than any screen.
pub const COLUMN_WIDTH_MIN: u16 = 32;
pub const COLUMN_WIDTH_MAX: u16 = 1200;

/// The id the remembered views are exported under.
pub const FOLDER_VIEWS_FILE_ID: &str = "folder-views";

/// The document format this build writes and reads.
pub const FOLDER_VIEWS_VERSION: u32 = 1;

/// The key the latest document is stored under, in its own file.
pub const FOLDER_VIEWS_KEY: &str = "folderViews";

/// The key the document of the run before is kept under.
pub const FOLDER_VIEWS_PREVIOUS_KEY: &str = "folderViewsPrevious";

/// The widths, in pixels, a folder's list columns were dragged to. `None` is the column's own
/// width. Name is not here: it is the flexible column and takes what the others leave. A width
/// applies only where its column is listed, so one set serves every layout (a folder, a Git
/// working tree, S3, the Trash).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ListColumnWidths {
    pub size: Option<u16>,
    pub modified: Option<u16>,
    pub kind: Option<u16>,
    pub git: Option<u16>,
    pub storage_class: Option<u16>,
    /// Where an item was trashed from (the Trash's own column).
    pub original: Option<u16>,
    /// When an item was trashed (the Trash's own column).
    pub deleted: Option<u16>,
}

impl ListColumnWidths {
    fn all(&self) -> [Option<u16>; 7] {
        [
            self.size,
            self.modified,
            self.kind,
            self.git,
            self.storage_class,
            self.original,
            self.deleted,
        ]
    }

    fn all_mut(&mut self) -> [&mut Option<u16>; 7] {
        [
            &mut self.size,
            &mut self.modified,
            &mut self.kind,
            &mut self.git,
            &mut self.storage_class,
            &mut self.original,
            &mut self.deleted,
        ]
    }

    /// Whether every column is at its own width, so there is nothing to remember.
    pub fn is_default(&self) -> bool {
        self.all().iter().all(Option::is_none)
    }

    fn in_range(&self) -> bool {
        self.all()
            .into_iter()
            .flatten()
            .all(|width| (COLUMN_WIDTH_MIN..=COLUMN_WIDTH_MAX).contains(&width))
    }

    fn clamped(mut self) -> Self {
        for width in self.all_mut() {
            *width = width.map(|width| width.clamp(COLUMN_WIDTH_MIN, COLUMN_WIDTH_MAX));
        }
        self
    }
}

/// What one folder remembers. A field that is `None` was never chosen here, and the window's own
/// choice shows for it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct FolderView {
    #[serde(default)]
    pub mode: Option<ViewMode>,
    /// The sort and the grouping, which are one value (grouping is the first key of the sort).
    #[serde(default)]
    pub sort: Option<SortSpec>,
    /// Whether the folder lists hidden files.
    #[serde(default)]
    pub show_hidden: Option<bool>,
    /// The grid's icon size in pixels, between `ICON_SIZE_MIN` and `ICON_SIZE_MAX`.
    #[serde(default)]
    #[ts(type = "number | null")]
    pub icon_size: Option<u32>,
    /// The widths the list's columns were dragged to in this folder (D176). Never an all-default
    /// set: a folder whose columns are all at their own widths remembers none.
    #[serde(default)]
    pub column_widths: Option<ListColumnWidths>,
}

impl FolderView {
    fn is_empty(&self) -> bool {
        self.mode.is_none()
            && self.sort.is_none()
            && self.show_hidden.is_none()
            && self.icon_size.is_none()
            && self.column_widths.is_none()
    }

    /// The form that is stored: widths held to their bounds, and a set with no width in it gone.
    fn normalised(mut self) -> Self {
        self.column_widths = self
            .column_widths
            .map(ListColumnWidths::clamped)
            .filter(|widths| !widths.is_default());
        self
    }

    fn valid(&self) -> bool {
        !self.is_empty()
            && self
                .icon_size
                .is_none_or(|size| (ICON_SIZE_MIN..=ICON_SIZE_MAX).contains(&size))
    }
}

/// What a window asks to be remembered: a field that is `None` leaves what is remembered alone.
/// `column_widths` replaces the folder's whole set of widths, so a set with no width in it
/// (every column at its own) makes the folder forget its widths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct FolderViewPatch {
    #[serde(default)]
    pub mode: Option<ViewMode>,
    #[serde(default)]
    pub sort: Option<SortSpec>,
    #[serde(default)]
    pub show_hidden: Option<bool>,
    #[serde(default)]
    #[ts(type = "number | null")]
    pub icon_size: Option<u32>,
    #[serde(default)]
    pub column_widths: Option<ListColumnWidths>,
}

/// One remembered folder, keyed by its location's `uri`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct FolderViewEntry {
    pub key: String,
    pub view: FolderView,
}

/// A folder whose memory changed: `view` is what it remembers now, `None` when it forgot (reset,
/// or dropped to stay within the bound).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct FolderViewChange {
    pub key: String,
    pub view: Option<FolderView>,
}

/// Every remembered folder and the revision it is at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct FolderViewsSnapshot {
    #[ts(type = "number")]
    pub revision: u64,
    pub folders: Vec<FolderViewEntry>,
}

/// Sent to every window after a change: the revision it made and the folders it touched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct FolderViewsChanged {
    #[ts(type = "number")]
    pub revision: u64,
    pub changes: Vec<FolderViewChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FolderViewsError {
    #[error("a folder's location must be between 1 and {MAX_KEY_BYTES} bytes, with no control characters")]
    BadKey,
    #[error("the icon size must be between {ICON_SIZE_MIN} and {ICON_SIZE_MAX}")]
    BadIconSize,
    #[error("a column width must be between {COLUMN_WIDTH_MIN} and {COLUMN_WIDTH_MAX} pixels")]
    BadColumnWidth,
    #[error("at most {MAX_FOLDERS} folders can be remembered")]
    TooMany,
    #[error("the location `{0}` is listed more than once")]
    Duplicate(String),
}

fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= MAX_KEY_BYTES && !key.chars().any(char::is_control)
}

/// The saved form: the folders in the order they were last written, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FolderViewsDocument {
    pub version: u32,
    pub folders: Vec<FolderViewEntry>,
}

impl FolderViewsDocument {
    pub fn new(folders: Vec<FolderViewEntry>) -> Self {
        Self {
            version: FOLDER_VIEWS_VERSION,
            folders,
        }
    }
}

/// The remembered views. Not shared: the plugin owns one behind its writer lock.
#[derive(Debug, Clone, Default)]
pub struct FolderViews {
    /// Oldest write first.
    entries: Vec<FolderViewEntry>,
    revision: u64,
}

impl FolderViews {
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads a saved document. Whatever cannot be right is left out rather than refused: a
    /// location that is not valid, a folder that remembers nothing, a repeated location (the later
    /// one wins) and anything over the bound (the oldest go).
    pub fn from_document(document: FolderViewsDocument) -> Self {
        let mut entries: Vec<FolderViewEntry> = Vec::with_capacity(document.folders.len());
        for mut entry in document.folders {
            entry.view = entry.view.normalised();
            if !valid_key(&entry.key) || !entry.view.valid() {
                continue;
            }
            entries.retain(|e| e.key != entry.key);
            entries.push(entry);
        }
        if entries.len() > MAX_FOLDERS {
            entries.drain(..entries.len() - MAX_FOLDERS);
        }
        Self {
            entries,
            revision: 0,
        }
    }

    pub fn to_document(&self) -> FolderViewsDocument {
        FolderViewsDocument::new(self.entries.clone())
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn snapshot(&self) -> FolderViewsSnapshot {
        FolderViewsSnapshot {
            revision: self.revision,
            folders: self.entries.clone(),
        }
    }

    pub fn get(&self, key: &str) -> Option<FolderView> {
        self.entries.iter().find(|e| e.key == key).map(|e| e.view)
    }

    fn bump(&mut self, changes: Vec<FolderViewChange>) -> Option<FolderViewsChanged> {
        if changes.is_empty() {
            return None;
        }
        self.revision += 1;
        Some(FolderViewsChanged {
            revision: self.revision,
            changes,
        })
    }

    /// Remembers `patch` for the folder at `key`, on top of what it already remembers, and makes
    /// it the most recently written. Remembering what is already remembered changes nothing (no
    /// revision, `None`). Going over the bound drops the folders written longest ago, reported as
    /// forgotten in the same change.
    pub fn remember(
        &mut self,
        key: &str,
        patch: FolderViewPatch,
    ) -> Result<Option<FolderViewsChanged>, FolderViewsError> {
        if !valid_key(key) {
            return Err(FolderViewsError::BadKey);
        }
        if patch
            .icon_size
            .is_some_and(|size| !(ICON_SIZE_MIN..=ICON_SIZE_MAX).contains(&size))
        {
            return Err(FolderViewsError::BadIconSize);
        }
        if patch.column_widths.is_some_and(|widths| !widths.in_range()) {
            return Err(FolderViewsError::BadColumnWidth);
        }
        let existing = self.entries.iter().position(|e| e.key == key);
        let before = existing.map(|i| self.entries[i].view);
        let view = FolderView {
            mode: patch.mode.or(before.and_then(|v| v.mode)),
            sort: patch.sort.or(before.and_then(|v| v.sort)),
            show_hidden: patch.show_hidden.or(before.and_then(|v| v.show_hidden)),
            icon_size: patch.icon_size.or(before.and_then(|v| v.icon_size)),
            column_widths: patch.column_widths.or(before.and_then(|v| v.column_widths)),
        }
        .normalised();
        if view.is_empty() {
            // Clearing the last thing a folder remembered forgets the folder.
            return Ok(existing.and_then(|_| self.forget(key)));
        }
        if before == Some(view) {
            return Ok(None);
        }
        if let Some(index) = existing {
            self.entries.remove(index);
        }
        self.entries.push(FolderViewEntry {
            key: key.to_owned(),
            view,
        });
        let mut changes = vec![FolderViewChange {
            key: key.to_owned(),
            view: Some(view),
        }];
        if self.entries.len() > MAX_FOLDERS {
            let over = self.entries.len() - MAX_FOLDERS;
            changes.extend(self.entries.drain(..over).map(|e| FolderViewChange {
                key: e.key,
                view: None,
            }));
        }
        Ok(self.bump(changes))
    }

    /// Forgets the folder at `key` (Reset This Folder's View). `None` when it remembered nothing.
    pub fn forget(&mut self, key: &str) -> Option<FolderViewsChanged> {
        let index = self.entries.iter().position(|e| e.key == key)?;
        let gone = self.entries.remove(index);
        self.bump(vec![FolderViewChange {
            key: gone.key,
            view: None,
        }])
    }

    /// Replaces everything remembered with `folders` (an import), validated as a normal write
    /// would be: refused, not trimmed, when a location is invalid or repeated or there are too
    /// many. The change names every folder that differs, so a window follows it as any other.
    pub fn replace_all(
        &mut self,
        folders: Vec<FolderViewEntry>,
    ) -> Result<Option<FolderViewsChanged>, FolderViewsError> {
        check_folders(&folders)?;
        let folders: Vec<FolderViewEntry> = folders
            .into_iter()
            .map(|mut entry| {
                entry.view = entry.view.normalised();
                entry
            })
            .collect();
        let mut changes = Vec::new();
        let incoming: HashMap<&str, FolderView> =
            folders.iter().map(|e| (e.key.as_str(), e.view)).collect();
        for entry in &self.entries {
            match incoming.get(entry.key.as_str()) {
                Some(view) if *view == entry.view => {}
                Some(view) => changes.push(FolderViewChange {
                    key: entry.key.clone(),
                    view: Some(*view),
                }),
                None => changes.push(FolderViewChange {
                    key: entry.key.clone(),
                    view: None,
                }),
            }
        }
        for entry in &folders {
            if !self.entries.iter().any(|e| e.key == entry.key) {
                changes.push(FolderViewChange {
                    key: entry.key.clone(),
                    view: Some(entry.view),
                });
            }
        }
        // The order is part of what is replaced (it is what pruning goes by), but only a change
        // of content is announced.
        let reordered = self.entries != folders;
        self.entries = folders;
        if changes.is_empty() {
            if reordered {
                self.revision += 1;
                return Ok(Some(FolderViewsChanged {
                    revision: self.revision,
                    changes,
                }));
            }
            return Ok(None);
        }
        Ok(self.bump(changes))
    }
}

/// Checks `folders` as an import must: every location valid, no folder that remembers nothing, no
/// location twice, no more than `MAX_FOLDERS`.
fn check_folders(folders: &[FolderViewEntry]) -> Result<(), FolderViewsError> {
    if folders.len() > MAX_FOLDERS {
        return Err(FolderViewsError::TooMany);
    }
    let mut seen = std::collections::HashSet::new();
    for entry in folders {
        if !valid_key(&entry.key) || entry.view.normalised().is_empty() {
            return Err(FolderViewsError::BadKey);
        }
        if !entry.view.valid() {
            return Err(FolderViewsError::BadIconSize);
        }
        if entry.view.column_widths.is_some_and(|w| !w.in_range()) {
            return Err(FolderViewsError::BadColumnWidth);
        }
        if !seen.insert(entry.key.as_str()) {
            return Err(FolderViewsError::Duplicate(entry.key.clone()));
        }
    }
    Ok(())
}

/// Plans importing a document of remembered views over `current`: the same checks as a normal
/// write, a version from a newer build refused, keys this build does not have left out and noted.
/// One group, `folders`, counts the folders the import would add, change or forget.
pub fn plan_folder_views(current: &FolderViews, incoming: &Value) -> Result<FilePlan, BundleError> {
    let invalid = |message: String| BundleError::Invalid {
        file: FOLDER_VIEWS_FILE_ID.to_owned(),
        message,
    };
    let object = incoming
        .as_object()
        .ok_or_else(|| invalid("it is not an object".to_owned()))?;
    let version = object
        .get("version")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("it has no version".to_owned()))?;
    if version > u64::from(FOLDER_VIEWS_VERSION) {
        return Err(BundleError::NewerDocument {
            file: FOLDER_VIEWS_FILE_ID.to_owned(),
            found: version,
            supported: FOLDER_VIEWS_VERSION,
        });
    }
    let parsed: FolderViewsDocument =
        serde_json::from_value(incoming.clone()).map_err(|e| invalid(e.to_string()))?;
    check_folders(&parsed.folders).map_err(|e| invalid(e.to_string()))?;
    let document = FolderViewsDocument::new(
        parsed
            .folders
            .into_iter()
            .map(|mut entry| {
                entry.view = entry.view.normalised();
                entry
            })
            .collect(),
    );
    let document_value = serde_json::to_value(&document).map_err(|e| invalid(e.to_string()))?;

    let mut probe = current.clone();
    let changed = probe
        .replace_all(document.folders)
        .map_err(|e| invalid(e.to_string()))?
        .map_or(0, |c| c.changes.len());
    let unknown = unknown_paths(incoming, &document_value);
    Ok(FilePlan {
        document: document_value,
        changes: if changed == 0 {
            Vec::new()
        } else {
            vec![ChangeGroup {
                file: FOLDER_VIEWS_FILE_ID.to_owned(),
                group: "folders".to_owned(),
                count: changed as u32,
            }]
        },
        warnings: if unknown.is_empty() {
            Vec::new()
        } else {
            vec![ImportWarning::UnknownKeys {
                file: FOLDER_VIEWS_FILE_ID.to_owned(),
                keys: unknown,
            }]
        },
    })
}

/// Where the remembered views live. The app implements it over its key-value file.
pub trait FolderViewsStorage: Send + Sync {
    /// The saved document, or `None` when nothing usable was saved.
    fn load(&self) -> Result<Option<FolderViewsDocument>, StorageError>;
    fn save(&self, document: &FolderViewsDocument) -> Result<(), StorageError>;
}

/// Remembered views kept in memory: for tests, and for an app that cannot open the file.
#[derive(Default)]
pub struct MemoryFolderViews {
    saved: Mutex<Option<FolderViewsDocument>>,
}

impl MemoryFolderViews {
    pub fn saved(&self) -> Option<FolderViewsDocument> {
        self.saved.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl FolderViewsStorage for MemoryFolderViews {
    fn load(&self) -> Result<Option<FolderViewsDocument>, StorageError> {
        Ok(self.saved())
    }

    fn save(&self, document: &FolderViewsDocument) -> Result<(), StorageError> {
        *self.saved.lock().unwrap_or_else(|e| e.into_inner()) = Some(document.clone());
        Ok(())
    }
}

/// The document storage: the first save of each run rotates the latest document into the
/// previous key, loading tries the latest then the previous, and a copy that cannot be read (or
/// that a newer build wrote) is set aside once. Remembered views that cannot be read are never
/// fatal: the folders just remember nothing.
pub struct FolderViewsPersistence<K: KeyValue> {
    kv: K,
    rotated: AtomicBool,
    set_aside: AtomicBool,
}

impl<K: KeyValue> FolderViewsPersistence<K> {
    pub fn new(kv: K) -> Self {
        Self {
            kv,
            rotated: AtomicBool::new(false),
            set_aside: AtomicBool::new(false),
        }
    }

    fn corrupt(&self, key: &str, why: &str) {
        log::warn!("the stored `{key}` folder views are unusable ({why}); trying the next copy");
        if !self.set_aside.swap(true, Ordering::AcqRel) {
            match self.kv.set_aside() {
                Some(name) => log::warn!("kept a copy of the folder views file as `{name}`"),
                None => log::warn!("could not keep a copy of the unreadable folder views file"),
            }
        }
    }

    fn read(&self, key: &str) -> Option<Result<FolderViewsDocument, String>> {
        let value = self.kv.get(key)?;
        Some(
            serde_json::from_value::<FolderViewsDocument>(value)
                .map_err(|e| e.to_string())
                .and_then(|doc| {
                    if doc.version > FOLDER_VIEWS_VERSION {
                        Err(format!(
                            "the document is version {}, this build reads version {FOLDER_VIEWS_VERSION}",
                            doc.version
                        ))
                    } else {
                        Ok(doc)
                    }
                }),
        )
    }
}

impl<K: KeyValue> FolderViewsStorage for FolderViewsPersistence<K> {
    fn load(&self) -> Result<Option<FolderViewsDocument>, StorageError> {
        if self.kv.was_unreadable() {
            self.set_aside.store(true, Ordering::Release);
        }
        for key in [FOLDER_VIEWS_KEY, FOLDER_VIEWS_PREVIOUS_KEY] {
            match self.read(key) {
                None => {}
                Some(Ok(document)) => return Ok(Some(document)),
                Some(Err(why)) => self.corrupt(key, &why),
            }
        }
        Ok(None)
    }

    fn save(&self, document: &FolderViewsDocument) -> Result<(), StorageError> {
        let value = serde_json::to_value(document).map_err(|e| StorageError::Io(e.to_string()))?;
        if !self.rotated.swap(true, Ordering::AcqRel) {
            // The first save of the run: what is on disk now is how the run started. An unusable
            // copy is not worth keeping over an earlier good `previous`.
            if let Some(Ok(_)) = self.read(FOLDER_VIEWS_KEY) {
                if let Some(current) = self.kv.get(FOLDER_VIEWS_KEY) {
                    self.kv.set(FOLDER_VIEWS_PREVIOUS_KEY, current);
                }
            }
        }
        self.kv.set(FOLDER_VIEWS_KEY, value);
        self.kv.save().map_err(StorageError::Io)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use waypoint_vfs::{GroupBy, SortKey};

    use super::*;

    fn grid() -> FolderViewPatch {
        FolderViewPatch {
            mode: Some(ViewMode::Grid),
            sort: None,
            ..Default::default()
        }
    }

    fn by_size() -> SortSpec {
        SortSpec {
            key: SortKey::Size,
            descending: true,
            directories_first: true,
            group_by: GroupBy::None,
        }
    }

    fn key(i: usize) -> String {
        format!("file:///folders/{i}")
    }

    #[test]
    fn remembering_a_folder_starts_the_revision_and_names_the_folder() {
        let mut views = FolderViews::new();
        let changed = views.remember("file:///a", grid()).unwrap().unwrap();
        assert_eq!(changed.revision, 1);
        assert_eq!(
            changed.changes,
            vec![FolderViewChange {
                key: "file:///a".into(),
                view: Some(FolderView {
                    mode: Some(ViewMode::Grid),
                    ..FolderView::default()
                })
            }]
        );
        assert_eq!(views.revision(), 1);
        assert_eq!(views.get("file:///a").unwrap().mode, Some(ViewMode::Grid));
    }

    #[test]
    fn a_patch_keeps_what_the_folder_already_remembers() {
        let mut views = FolderViews::new();
        views.remember("file:///a", grid()).unwrap();
        views
            .remember(
                "file:///a",
                FolderViewPatch {
                    mode: None,
                    sort: Some(by_size()),
                    ..Default::default()
                },
            )
            .unwrap();
        let view = views.get("file:///a").unwrap();
        assert_eq!(view.mode, Some(ViewMode::Grid));
        assert_eq!(view.sort, Some(by_size()));
        assert_eq!(views.revision(), 2);
        assert_eq!(views.len(), 1);
    }

    #[test]
    fn remembering_what_is_already_remembered_changes_nothing() {
        let mut views = FolderViews::new();
        views.remember("file:///a", grid()).unwrap();
        assert_eq!(views.remember("file:///a", grid()).unwrap(), None);
        assert_eq!(
            views
                .remember("file:///b", FolderViewPatch::default())
                .unwrap(),
            None
        );
        assert_eq!(views.revision(), 1);
        assert_eq!(views.len(), 1);
    }

    #[test]
    fn forgetting_a_folder_is_a_change_and_forgetting_a_stranger_is_not() {
        let mut views = FolderViews::new();
        views.remember("file:///a", grid()).unwrap();
        let changed = views.forget("file:///a").unwrap();
        assert_eq!(changed.revision, 2);
        assert_eq!(changed.changes[0].view, None);
        assert!(views.is_empty());
        assert_eq!(views.forget("file:///a"), None);
        assert_eq!(views.revision(), 2);
    }

    #[test]
    fn a_location_that_is_empty_too_long_or_has_control_characters_is_refused() {
        let mut views = FolderViews::new();
        for bad in [
            String::new(),
            "x".repeat(MAX_KEY_BYTES + 1),
            "file:///a\nb".to_owned(),
        ] {
            assert_eq!(
                views.remember(&bad, grid()).unwrap_err(),
                FolderViewsError::BadKey
            );
        }
        assert!(views.remember(&"x".repeat(MAX_KEY_BYTES), grid()).is_ok());
        assert_eq!(views.revision(), 1);
    }

    #[test]
    fn going_over_the_bound_drops_the_folder_written_longest_ago_in_the_same_change() {
        let mut views = FolderViews::new();
        for i in 0..MAX_FOLDERS {
            views.remember(&key(i), grid()).unwrap();
        }
        assert_eq!(views.len(), MAX_FOLDERS);
        // Writing the oldest again makes it the newest, so the next one over drops folder 1.
        views
            .remember(
                &key(0),
                FolderViewPatch {
                    mode: None,
                    sort: Some(by_size()),
                    ..Default::default()
                },
            )
            .unwrap();
        let changed = views
            .remember("file:///one-too-many", grid())
            .unwrap()
            .unwrap();
        assert_eq!(views.len(), MAX_FOLDERS);
        assert_eq!(changed.changes.len(), 2);
        assert_eq!(changed.changes[1].key, key(1));
        assert_eq!(changed.changes[1].view, None);
        assert!(views.get(&key(1)).is_none());
        assert!(views.get(&key(0)).is_some());
        assert_eq!(changed.revision, views.revision());
    }

    #[test]
    fn a_loaded_document_is_cleaned_and_trimmed_to_the_bound() {
        let mut folders: Vec<FolderViewEntry> = (0..MAX_FOLDERS + 5)
            .map(|i| FolderViewEntry {
                key: key(i),
                view: FolderView {
                    mode: Some(ViewMode::List),
                    sort: None,
                    ..Default::default()
                },
            })
            .collect();
        folders.push(FolderViewEntry {
            key: String::new(),
            view: FolderView {
                mode: Some(ViewMode::List),
                sort: None,
                ..Default::default()
            },
        });
        folders.push(FolderViewEntry {
            key: "file:///nothing".into(),
            view: FolderView::default(),
        });
        // A repeat: the later one wins and counts as the newest.
        folders.push(FolderViewEntry {
            key: key(10),
            view: FolderView {
                mode: Some(ViewMode::Grid),
                sort: None,
                ..Default::default()
            },
        });
        let views = FolderViews::from_document(FolderViewsDocument::new(folders));
        assert_eq!(views.len(), MAX_FOLDERS);
        assert_eq!(views.revision(), 0);
        assert_eq!(views.get(&key(10)).unwrap().mode, Some(ViewMode::Grid));
        assert!(views.get(&key(0)).is_none());
        assert!(views.get("file:///nothing").is_none());
        assert_eq!(views.to_document().folders.last().unwrap().key, key(10));
    }

    #[test]
    fn replacing_everything_names_each_folder_that_differs() {
        let mut views = FolderViews::new();
        views.remember("file:///kept", grid()).unwrap();
        views.remember("file:///changed", grid()).unwrap();
        views.remember("file:///gone", grid()).unwrap();
        let list = FolderView {
            mode: Some(ViewMode::List),
            sort: None,
            ..Default::default()
        };
        let changed = views
            .replace_all(vec![
                FolderViewEntry {
                    key: "file:///kept".into(),
                    view: FolderView {
                        mode: Some(ViewMode::Grid),
                        sort: None,
                        ..Default::default()
                    },
                },
                FolderViewEntry {
                    key: "file:///changed".into(),
                    view: list,
                },
                FolderViewEntry {
                    key: "file:///new".into(),
                    view: list,
                },
            ])
            .unwrap()
            .unwrap();
        assert_eq!(changed.revision, 4);
        let named: Vec<_> = changed.changes.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(named, ["file:///changed", "file:///gone", "file:///new"]);
        assert_eq!(views.len(), 3);
    }

    #[test]
    fn replacing_with_what_is_there_changes_nothing_and_a_bad_list_is_refused_whole() {
        let mut views = FolderViews::new();
        views.remember("file:///a", grid()).unwrap();
        let same = views.to_document().folders;
        assert_eq!(views.replace_all(same.clone()).unwrap(), None);
        assert_eq!(views.revision(), 1);

        let twice = vec![same[0].clone(), same[0].clone()];
        assert_eq!(
            views.replace_all(twice).unwrap_err(),
            FolderViewsError::Duplicate("file:///a".into())
        );
        let too_many: Vec<FolderViewEntry> = (0..MAX_FOLDERS + 1)
            .map(|i| FolderViewEntry {
                key: key(i),
                view: same[0].view,
            })
            .collect();
        assert_eq!(
            views.replace_all(too_many).unwrap_err(),
            FolderViewsError::TooMany
        );
        assert_eq!(views.len(), 1);
        assert_eq!(views.revision(), 1);
    }

    #[test]
    fn hidden_files_and_icon_size_are_remembered_beside_the_view_and_merge_like_it() {
        let mut views = FolderViews::new();
        views
            .remember(
                "file:///a",
                FolderViewPatch {
                    show_hidden: Some(true),
                    ..Default::default()
                },
            )
            .unwrap();
        views
            .remember(
                "file:///a",
                FolderViewPatch {
                    icon_size: Some(128),
                    mode: Some(ViewMode::Grid),
                    ..Default::default()
                },
            )
            .unwrap();
        let view = views.get("file:///a").unwrap();
        assert_eq!(view.show_hidden, Some(true));
        assert_eq!(view.icon_size, Some(128));
        assert_eq!(view.mode, Some(ViewMode::Grid));
        // Turning hidden files back off is a choice too, not a return to nothing.
        views
            .remember(
                "file:///a",
                FolderViewPatch {
                    show_hidden: Some(false),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(views.get("file:///a").unwrap().show_hidden, Some(false));
        assert_eq!(views.revision(), 3);
    }

    #[test]
    fn an_icon_size_outside_the_grids_range_is_refused_and_dropped_when_loaded() {
        let mut views = FolderViews::new();
        for size in [ICON_SIZE_MIN - 1, ICON_SIZE_MAX + 1] {
            assert_eq!(
                views
                    .remember(
                        "file:///a",
                        FolderViewPatch {
                            icon_size: Some(size),
                            ..Default::default()
                        },
                    )
                    .unwrap_err(),
                FolderViewsError::BadIconSize
            );
        }
        assert!(views.is_empty());
        let loaded = FolderViews::from_document(FolderViewsDocument::new(vec![FolderViewEntry {
            key: "file:///a".into(),
            view: FolderView {
                icon_size: Some(9999),
                ..Default::default()
            },
        }]));
        assert!(loaded.is_empty());
        let plan = plan_folder_views(
            &FolderViews::new(),
            &json!({"version": 1, "folders": [{"key": "file:///a", "view": {"iconSize": 5}}]}),
        );
        assert_eq!(plan.unwrap_err().code(), "invalid");
    }

    #[test]
    fn the_snapshot_carries_the_revision_and_the_folders_oldest_first() {
        let mut views = FolderViews::new();
        views.remember("file:///a", grid()).unwrap();
        views.remember("file:///b", grid()).unwrap();
        let snapshot = views.snapshot();
        assert_eq!(snapshot.revision, 2);
        let keys: Vec<_> = snapshot.folders.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, ["file:///a", "file:///b"]);
    }

    #[test]
    fn the_wire_form_is_camel_case_with_nulls_for_what_was_never_chosen() {
        let view = FolderView {
            mode: Some(ViewMode::Grid),
            sort: None,
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(view).unwrap(),
            json!({
                "mode": "grid",
                "sort": null,
                "showHidden": null,
                "iconSize": null,
                "columnWidths": null,
            })
        );
        let read: FolderView = serde_json::from_value(json!({"mode": "list"})).unwrap();
        assert_eq!(read.sort, None);
        assert_eq!(read.column_widths, None);
    }

    fn widths(f: impl FnOnce(&mut ListColumnWidths)) -> FolderViewPatch {
        let mut set = ListColumnWidths::default();
        f(&mut set);
        FolderViewPatch {
            column_widths: Some(set),
            ..Default::default()
        }
    }

    #[test]
    fn column_widths_are_remembered_per_folder_in_camel_case() {
        let mut views = FolderViews::new();
        views
            .remember(
                "file:///a",
                widths(|w| {
                    w.size = Some(120);
                    w.storage_class = Some(150);
                }),
            )
            .unwrap();
        views
            .remember("file:///b", widths(|w| w.modified = Some(200)))
            .unwrap();
        let a = views.get("file:///a").unwrap().column_widths.unwrap();
        assert_eq!(
            (a.size, a.storage_class, a.modified),
            (Some(120), Some(150), None)
        );
        let b = views.get("file:///b").unwrap().column_widths.unwrap();
        assert_eq!((b.size, b.modified), (None, Some(200)));
        assert_eq!(views.get("file:///c"), None);
        let json = serde_json::to_value(views.get("file:///a").unwrap()).unwrap();
        assert_eq!(json["columnWidths"]["storageClass"], 150);
        assert!(json["columnWidths"]["modified"].is_null());
        assert!(json["mode"].is_null());
    }

    #[test]
    fn a_patch_without_widths_leaves_them_and_a_patch_with_widths_replaces_the_set() {
        let mut views = FolderViews::new();
        views
            .remember("file:///a", widths(|w| w.size = Some(120)))
            .unwrap();
        views.remember("file:///a", grid()).unwrap();
        let view = views.get("file:///a").unwrap();
        assert_eq!(view.mode, Some(ViewMode::Grid));
        assert_eq!(view.column_widths.unwrap().size, Some(120));

        views
            .remember("file:///a", widths(|w| w.kind = Some(90)))
            .unwrap();
        let set = views.get("file:///a").unwrap().column_widths.unwrap();
        assert_eq!((set.size, set.kind), (None, Some(90)));
        assert_eq!(views.revision(), 3);

        // The same set again changes nothing.
        assert_eq!(
            views
                .remember("file:///a", widths(|w| w.kind = Some(90)))
                .unwrap(),
            None
        );
        assert_eq!(views.revision(), 3);
    }

    #[test]
    fn clearing_the_widths_keeps_what_else_the_folder_remembers_and_forgets_a_folder_with_nothing_left(
    ) {
        let mut views = FolderViews::new();
        views
            .remember("file:///a", widths(|w| w.size = Some(120)))
            .unwrap();
        views.remember("file:///a", grid()).unwrap();
        let cleared = views
            .remember("file:///a", widths(|_| {}))
            .unwrap()
            .unwrap();
        let view = cleared.changes[0].view.unwrap();
        assert_eq!(view.mode, Some(ViewMode::Grid));
        assert_eq!(view.column_widths, None);

        // Widths were all that `b` remembered: clearing them drops the record, as a reset does.
        views
            .remember("file:///b", widths(|w| w.size = Some(120)))
            .unwrap();
        let before = views.len();
        let cleared = views
            .remember("file:///b", widths(|_| {}))
            .unwrap()
            .unwrap();
        assert_eq!(cleared.changes[0].view, None);
        assert_eq!(views.len(), before - 1);
        assert_eq!(views.get("file:///b"), None);
        // A folder that remembers nothing has nothing to clear, and a set of defaults starts no record.
        assert_eq!(views.remember("file:///b", widths(|_| {})).unwrap(), None);
        assert_eq!(views.get("file:///b"), None);
        assert_eq!(views.len(), before - 1);
    }

    #[test]
    fn a_column_width_is_bounded_at_both_ends() {
        let mut views = FolderViews::new();
        for width in [COLUMN_WIDTH_MIN, COLUMN_WIDTH_MAX] {
            assert!(views
                .remember("file:///a", widths(|w| w.original = Some(width)))
                .is_ok());
        }
        let revision = views.revision();
        for width in [0, COLUMN_WIDTH_MIN - 1, COLUMN_WIDTH_MAX + 1, u16::MAX] {
            assert_eq!(
                views
                    .remember("file:///a", widths(|w| w.deleted = Some(width)))
                    .unwrap_err(),
                FolderViewsError::BadColumnWidth
            );
        }
        assert_eq!(views.revision(), revision);

        let loaded = FolderViews::from_document(FolderViewsDocument::new(vec![FolderViewEntry {
            key: "file:///a".into(),
            view: FolderView {
                column_widths: Some(ListColumnWidths {
                    size: Some(1),
                    kind: Some(u16::MAX),
                    ..Default::default()
                }),
                ..Default::default()
            },
        }]));
        let set = loaded.get("file:///a").unwrap().column_widths.unwrap();
        assert_eq!(
            (set.size, set.kind),
            (Some(COLUMN_WIDTH_MIN), Some(COLUMN_WIDTH_MAX))
        );
        let plan = plan_folder_views(
            &FolderViews::new(),
            &json!({"version": 1, "folders": [{"key": "file:///a", "view": {"columnWidths": {"size": 5}}}]}),
        );
        assert_eq!(plan.unwrap_err().code(), "invalid");
    }

    #[test]
    fn a_document_from_before_widths_loads_and_a_widths_only_folder_counts_toward_the_bound() {
        let old: FolderViewsDocument = serde_json::from_value(json!({
            "version": 1,
            "folders": [{"key": "file:///a", "view": {"mode": "grid", "sort": null}}],
        }))
        .unwrap();
        let mut views = FolderViews::from_document(old);
        assert_eq!(views.get("file:///a").unwrap().column_widths, None);

        // A set with no width in it is not a choice: a record of only that is left out when loaded.
        let empty = FolderViews::from_document(FolderViewsDocument::new(vec![FolderViewEntry {
            key: "file:///b".into(),
            view: FolderView {
                column_widths: Some(ListColumnWidths::default()),
                ..Default::default()
            },
        }]));
        assert!(empty.is_empty());

        for i in 0..MAX_FOLDERS {
            views
                .remember(&key(i), widths(|w| w.size = Some(100)))
                .unwrap();
        }
        assert_eq!(views.len(), MAX_FOLDERS);
        assert!(views.get("file:///a").is_none());
        assert!(views.get(&key(0)).is_some());
    }

    #[test]
    fn a_plan_carries_widths_and_refuses_a_record_of_nothing_but_defaults() {
        let current = FolderViews::new();
        let plan = plan(
            &current,
            json!({"version": 1, "folders": [
                {"key": "file:///a", "view": {"columnWidths": {"size": 120}}},
                {"key": "file:///b", "view": {"mode": "grid", "columnWidths": {}}},
            ]}),
        )
        .unwrap();
        assert_eq!(plan.changes[0].count, 2);
        assert_eq!(
            plan.document["folders"][0]["view"]["columnWidths"]["size"],
            120
        );
        assert!(plan.document["folders"][1]["view"]["columnWidths"].is_null());
        assert_eq!(
            plan_folder_views(
                &current,
                &json!({"version": 1, "folders": [{"key": "file:///a", "view": {"columnWidths": {}}}]}),
            )
            .unwrap_err()
            .code(),
            "invalid"
        );
    }

    fn plan(current: &FolderViews, incoming: Value) -> Result<FilePlan, BundleError> {
        plan_folder_views(current, &incoming)
    }

    #[test]
    fn a_plan_counts_the_folders_it_would_change_and_notes_unknown_keys() {
        let mut current = FolderViews::new();
        current.remember("file:///a", grid()).unwrap();
        let plan = plan(
            &current,
            json!({
                "version": 1,
                "folders": [
                    {"key": "file:///a", "view": {"mode": "grid", "sort": null}},
                    {"key": "file:///b", "view": {"mode": "list", "sort": null}},
                ],
                "extra": true,
            }),
        )
        .unwrap();
        assert_eq!(plan.changes.len(), 1);
        assert_eq!(plan.changes[0].group, "folders");
        assert_eq!(plan.changes[0].count, 1);
        assert_eq!(
            plan.warnings,
            vec![ImportWarning::UnknownKeys {
                file: FOLDER_VIEWS_FILE_ID.into(),
                keys: vec!["extra".into()]
            }]
        );
        // What would be applied is exactly the normalised document.
        assert_eq!(plan.document["folders"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_plan_of_what_is_already_there_has_no_changes() {
        let mut current = FolderViews::new();
        current.remember("file:///a", grid()).unwrap();
        let exported = serde_json::to_value(current.to_document()).unwrap();
        let plan = plan(&current, exported).unwrap();
        assert!(plan.changes.is_empty());
        assert!(plan.warnings.is_empty());
    }

    #[test]
    fn a_plan_refuses_what_a_normal_write_would() {
        let current = FolderViews::new();
        let refused = |incoming: Value| plan(&current, incoming).unwrap_err().code();
        assert_eq!(refused(json!([1])), "invalid");
        assert_eq!(refused(json!({"folders": []})), "invalid");
        assert_eq!(
            refused(json!({"version": 2, "folders": []})),
            "newer-format"
        );
        assert_eq!(
            refused(json!({"version": 1, "folders": [{"key": "", "view": {"mode": "grid"}}]})),
            "invalid"
        );
        assert_eq!(
            refused(json!({"version": 1, "folders": [{"key": "file:///a", "view": {}}]})),
            "invalid"
        );
        assert_eq!(
            refused(json!({"version": 1, "folders": [
                {"key": "file:///a", "view": {"mode": "grid"}},
                {"key": "file:///a", "view": {"mode": "list"}},
            ]})),
            "invalid"
        );
        assert_eq!(
            refused(
                json!({"version": 1, "folders": [{"key": "file:///a", "view": {"mode": "tiles"}}]})
            ),
            "invalid"
        );
    }

    #[derive(Default)]
    struct Memory {
        values: Mutex<HashMap<String, Value>>,
        aside: Mutex<u32>,
    }

    impl KeyValue for Arc<Memory> {
        fn get(&self, key: &str) -> Option<Value> {
            self.values.lock().unwrap().get(key).cloned()
        }
        fn set(&self, key: &str, value: Value) {
            self.values.lock().unwrap().insert(key.to_string(), value);
        }
        fn save(&self) -> Result<(), String> {
            Ok(())
        }
        fn set_aside(&self) -> Option<String> {
            *self.aside.lock().unwrap() += 1;
            Some("folder-views.json.corrupt-1".into())
        }
    }

    fn document(count: usize) -> FolderViewsDocument {
        FolderViewsDocument::new(
            (0..count)
                .map(|i| FolderViewEntry {
                    key: key(i),
                    view: FolderView {
                        mode: Some(ViewMode::Grid),
                        sort: None,
                        ..Default::default()
                    },
                })
                .collect(),
        )
    }

    #[test]
    fn a_saved_document_loads_back_and_nothing_saved_loads_nothing() {
        let memory = Arc::new(Memory::default());
        let storage = FolderViewsPersistence::new(memory);
        assert_eq!(storage.load().unwrap(), None);
        storage.save(&document(2)).unwrap();
        assert_eq!(storage.load().unwrap(), Some(document(2)));
    }

    #[test]
    fn a_corrupt_copy_falls_back_to_the_previous_and_is_set_aside_once() {
        let memory = Arc::new(Memory::default());
        memory.set(FOLDER_VIEWS_KEY, json!("not a document"));
        memory.set(
            FOLDER_VIEWS_PREVIOUS_KEY,
            serde_json::to_value(document(3)).unwrap(),
        );
        let storage = FolderViewsPersistence::new(memory.clone());
        assert_eq!(storage.load().unwrap(), Some(document(3)));
        assert_eq!(*memory.aside.lock().unwrap(), 1);

        let newer = Arc::new(Memory::default());
        newer.set(FOLDER_VIEWS_KEY, json!({"version": 99, "folders": []}));
        let storage = FolderViewsPersistence::new(newer.clone());
        assert_eq!(storage.load().unwrap(), None);
        assert_eq!(*newer.aside.lock().unwrap(), 1);
    }

    #[test]
    fn only_the_first_save_of_a_run_rotates_into_the_previous_copy() {
        let memory = Arc::new(Memory::default());
        memory.set(FOLDER_VIEWS_KEY, serde_json::to_value(document(1)).unwrap());
        let storage = FolderViewsPersistence::new(memory.clone());
        storage.save(&document(2)).unwrap();
        storage.save(&document(3)).unwrap();
        let previous: FolderViewsDocument =
            serde_json::from_value(memory.get(FOLDER_VIEWS_PREVIOUS_KEY).unwrap()).unwrap();
        assert_eq!(previous.folders.len(), 1, "previous is how the run started");
        let latest: FolderViewsDocument =
            serde_json::from_value(memory.get(FOLDER_VIEWS_KEY).unwrap()).unwrap();
        assert_eq!(latest.folders.len(), 3);
    }

    #[test]
    fn the_memory_storage_keeps_what_it_is_given() {
        let storage = MemoryFolderViews::default();
        assert_eq!(storage.load().unwrap(), None);
        storage.save(&document(1)).unwrap();
        assert_eq!(storage.saved(), Some(document(1)));
    }
}
