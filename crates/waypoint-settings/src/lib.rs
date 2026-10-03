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
mod model;
mod storage;

pub use accelerator::{
    validate_accelerator, ACCELERATOR_HINT, ACCELERATOR_MAX_LEN, DEFAULT_ACCELERATOR,
};
pub use model::{
    BlurLevel, ClickMode, ColourMode, DefaultView, DndSettings, DropActionRule, GeneralSettings,
    OsPreference, Settings, SettingsError, SettingsSnapshot, StartupMode, UiSettings,
    SPRING_LOAD_MAX_MS, SPRING_LOAD_MIN_MS,
};
pub use storage::{
    KeyValue, MemoryStorage, Persistence, SettingsDocument, SettingsStorage, StorageError,
    DOCUMENT_VERSION, PREVIOUS_KEY, SETTINGS_KEY,
};
