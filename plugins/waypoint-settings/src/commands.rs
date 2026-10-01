// The plugin's commands: read the settings, change them, and report that the plugin works
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{Runtime, State, WebviewWindow};
use waypoint_protocol::PluginStatus;
use waypoint_settings::{Settings, SettingsSnapshot};

use crate::error::Error;
use crate::state::SettingsStore;

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
