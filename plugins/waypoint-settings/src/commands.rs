// The plugin's commands: read the settings, change them, and report that the plugin works
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::Deserialize;
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

/// The `ui` settings a window changes; a field left out keeps whatever is in force.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiChange {
    pub action_bar: Option<bool>,
    pub action_bar_labels: Option<bool>,
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
    })
}
