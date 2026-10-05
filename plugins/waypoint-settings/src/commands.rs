// The plugin's commands: read the settings, change them, and report that the plugin works
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::Deserialize;
use tauri::{Manager, Runtime, State, WebviewWindow};
use waypoint_protocol::PluginStatus;
use waypoint_settings::{
    ExportReceipt, FolderViewPatch, FolderViewsSnapshot, ImportPreview, Settings, SettingsSnapshot,
};

use crate::error::Error;
use crate::folder_views::FolderViewsStore;
use crate::state::SettingsStore;
use crate::transfer::Transfers;

/// Reports that the settings work. They need nothing from the system, so this is always available.
#[tauri::command]
pub async fn get_status() -> PluginStatus {
    PluginStatus::available(vec!["settings".to_owned(), "events".to_owned()])
}

/// The settings in force and their revision.
#[tauri::command]
pub async fn get_settings<R: Runtime>(
    _window: WebviewWindow<R>,
    store: State<'_, SettingsStore<R>>,
) -> Result<SettingsSnapshot, Error> {
    Ok(store.snapshot())
}

/// Saves new settings and announces them to every window. Returns what is now in force; a value
/// out of range rejects with `{ kind: "invalid", message, field, min, max }` and changes nothing.
#[tauri::command]
pub async fn set_settings<R: Runtime>(
    _window: WebviewWindow<R>,
    store: State<'_, SettingsStore<R>>,
    settings: Settings,
) -> Result<SettingsSnapshot, Error> {
    store.set(settings)
}

/// Asks where to save with the system's save dialog, then writes the settings there: one `.json`
/// when one configuration file is exported and a `.zip` when there are several. `None` when the
/// dialog was closed. The page sends only its time zone offset, so the suggested name carries the
/// person's own date; the path comes from the dialog and goes no further than this command.
#[tauri::command]
pub async fn export_settings<R: Runtime>(
    window: WebviewWindow<R>,
    transfers: State<'_, Transfers>,
    utc_offset_minutes: i32,
) -> Result<Option<ExportReceipt>, Error> {
    let picker = transfers.picker().ok_or(Error::NoPicker)?;
    let version = window.app_handle().package_info().version.to_string();
    let bundle = transfers.export(&version, utc_offset_minutes)?;
    let label = window.label().to_owned();
    // The dialog blocks until it is answered; this command runs off the main thread.
    let Some(path) = picker.save(&label, &bundle.suggested_name, bundle.kind) else {
        return Ok(None);
    };
    transfers.write(&path, &bundle).map(Some)
}

/// Asks which file to read with the system's open dialog and plans importing it, touching
/// nothing: what would change, and what to know first. `None` when the dialog was closed. The
/// file is kept in Rust for `apply_settings_import`.
#[tauri::command]
pub async fn plan_settings_import<R: Runtime>(
    window: WebviewWindow<R>,
    transfers: State<'_, Transfers>,
) -> Result<Option<ImportPreview>, Error> {
    let picker = transfers.picker().ok_or(Error::NoPicker)?;
    let Some(path) = picker.open(window.label()) else {
        return Ok(None);
    };
    transfers.plan_path(&path).map(Some)
}

/// Applies the plan just made, all or nothing, through the same path as any other change: one
/// revision, one event to every window. The file is checked again; nothing the page holds is
/// applied. Returns what is now in force.
#[tauri::command]
pub async fn apply_settings_import<R: Runtime>(
    _window: WebviewWindow<R>,
    store: State<'_, SettingsStore<R>>,
    transfers: State<'_, Transfers>,
    plan_id: u64,
) -> Result<SettingsSnapshot, Error> {
    transfers.apply(plan_id)?;
    Ok(store.snapshot())
}

/// The `ui` settings a window changes; a field left out keeps whatever is in force.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiChange {
    pub action_bar: Option<bool>,
    pub action_bar_labels: Option<bool>,
    pub git_column: Option<bool>,
}

/// Changes only the `ui` settings named in `change`, on top of what is in force now, and
/// announces them to every window. The main windows may call this and not `set_settings`: they
/// hold a copy of the whole document that another window may have moved on from.
#[tauri::command]
pub async fn set_ui_settings<R: Runtime>(
    _window: WebviewWindow<R>,
    store: State<'_, SettingsStore<R>>,
    change: UiChange,
) -> Result<SettingsSnapshot, Error> {
    store.update_ui(|ui| {
        if let Some(shown) = change.action_bar {
            ui.action_bar = shown;
        }
        if let Some(shown) = change.action_bar_labels {
            ui.action_bar_labels = shown;
        }
        if let Some(shown) = change.git_column {
            ui.git_column = shown;
        }
    })
}

/// Every remembered folder view and the revision they are at.
#[tauri::command]
pub async fn get_folder_views<R: Runtime>(
    _window: WebviewWindow<R>,
    store: State<'_, FolderViewsStore<R>>,
) -> Result<FolderViewsSnapshot, Error> {
    Ok(store.snapshot())
}

/// Remembers the view, sort or grouping a window chose for the folder at `key` (its location's
/// `uri`), on top of what that folder already remembers, and announces it to every window. A
/// location that cannot be one rejects with `{ kind: "invalid", message }`. Returns the revision
/// in force.
#[tauri::command]
pub async fn remember_folder_view<R: Runtime>(
    _window: WebviewWindow<R>,
    store: State<'_, FolderViewsStore<R>>,
    key: String,
    patch: FolderViewPatch,
) -> Result<u64, Error> {
    store.remember(&key, patch)
}

/// Makes the folder at `key` forget its own view (View ▸ Reset This Folder's View): it shows the
/// window's again. Returns the revision in force.
#[tauri::command]
pub async fn reset_folder_view<R: Runtime>(
    _window: WebviewWindow<R>,
    store: State<'_, FolderViewsStore<R>>,
    key: String,
) -> Result<u64, Error> {
    store.forget(&key)
}
