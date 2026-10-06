// Waypoint's application settings: the typed document, its validation and its storage
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// This crate owns everything the Settings window edits except the operations settings, which the
// operations plugin owns (`OpsSettings`, one writer per piece of state). It has no `tauri`
// dependency: the plugin in `plugins/waypoint-settings` is the adapter, and the app supplies the
// key-value file the document is saved in.

mod accelerator;
mod bundle;
mod folder_views;
mod import;
mod model;
mod storage;

pub use accelerator::{
    validate_accelerator, ACCELERATOR_HINT, ACCELERATOR_MAX_LEN, DEFAULT_ACCELERATOR,
};
pub use bundle::{
    export as export_bundle, read as read_bundle, valid_id as valid_file_id, Bundle, BundleError,
    BundleKind, ExportFile, ExportMeta, ExportedBundle, BUNDLE_FORMAT, BUNDLE_VERSION,
    MANIFEST_NAME, MAX_ENTRIES, MAX_FILE_BYTES, MAX_TOTAL_BYTES,
};
pub use folder_views::{
    plan_folder_views, FolderView, FolderViewChange, FolderViewEntry, FolderViewPatch, FolderViews,
    FolderViewsChanged, FolderViewsDocument, FolderViewsError, FolderViewsPersistence,
    FolderViewsSnapshot, FolderViewsStorage, MemoryFolderViews, FOLDER_VIEWS_FILE_ID,
    FOLDER_VIEWS_KEY, FOLDER_VIEWS_PREVIOUS_KEY, FOLDER_VIEWS_VERSION, ICON_SIZE_MAX,
    ICON_SIZE_MIN, MAX_FOLDERS, MAX_KEY_BYTES,
};
pub use import::{
    apply_import, change_groups, differing_paths, plan_import, plan_settings, unknown_paths,
    ApplyError, ChangeGroup, ConfigFile, ExportReceipt, FilePlan, ImportPlan, ImportPreview,
    ImportWarning, PlannedImport, SETTINGS_FILE_ID,
};
pub use model::{
    BlurLevel, ClickMode, ColourMode, DefaultView, DndSettings, DropActionRule,
    ExperimentalSettings, GeneralSettings, ListColumnWidths, OsPreference, Settings, SettingsError,
    SettingsSnapshot, StartupMode, UiSettings, COLUMN_WIDTH_MAX, COLUMN_WIDTH_MIN,
    SPRING_LOAD_MAX_MS, SPRING_LOAD_MIN_MS,
};
pub use storage::{
    KeyValue, MemoryStorage, Persistence, SettingsDocument, SettingsStorage, StorageError,
    DOCUMENT_VERSION, PREVIOUS_KEY, SETTINGS_KEY,
};
