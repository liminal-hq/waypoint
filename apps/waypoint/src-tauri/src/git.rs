// Composes the Git plugin: puts its status on the file system plugin's listings and follows the Settings switch
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The Git plugin knows repositories and calls no other plugin (A4); the file system plugin knows
// listings and nothing of Git. This is where they meet: the plugin's overlay becomes the file
// system plugin's, so the listing of a folder in a working tree carries Git marks, and the Settings
// switch turns the whole status on and off.

use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_waypoint_git::Git;
use tauri_plugin_waypoint_settings::SettingsStore;
use tauri_plugin_waypoint_vfs::Vfs;
use waypoint_settings::Settings;

/// Whether the Git status is on under `settings`.
fn decorations(settings: &Settings) -> bool {
    settings.general.git_decorations
}

/// Connects the two plugins and applies the switch. Call it in `setup`, after both plugins exist.
pub fn wire(app: &AppHandle<Wry>) {
    let Some(git) = app.try_state::<Git>() else {
        return;
    };
    if let Some(vfs) = app.try_state::<Vfs>() {
        vfs.set_overlay(git.overlay());
    }
    git.set_enabled(decorations(&crate::settings::current(app)));
    if let Some(store) = app.try_state::<SettingsStore<Wry>>() {
        let handle = app.clone();
        store.on_change(move |settings| {
            if let Some(git) = handle.try_state::<Git>() {
                git.set_enabled(decorations(settings));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switch_is_the_general_settings_git_decorations() {
        let mut settings = Settings::default();
        assert!(decorations(&settings));
        settings.general.git_decorations = false;
        assert!(!decorations(&settings));
    }
}
