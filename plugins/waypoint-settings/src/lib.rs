// Registers the settings plugin: one document for every window, a change event, a status command
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
mod error;
mod state;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, Runtime,
};
use waypoint_settings::SettingsStorage;

pub use error::Error;
pub use state::{ChangeHook, SettingsStore};

/// Sent to every window after every change, with a `SettingsSnapshot` whose revision grows by one
/// each time. A window reads `get_settings` first and applies events with a higher revision.
pub const EVENT: &str = "waypoint-settings://changed";

/// Registers the plugin over a storage that is ready.
pub fn init<R: Runtime>(storage: Arc<dyn SettingsStorage>) -> TauriPlugin<R> {
    init_with(move |_| storage)
}

/// Registers the plugin with its storage made when the plugin is set up, which is when the app
/// exists: the file it saves in is opened through `tauri-plugin-store`, so register this after it.
pub fn init_with<R: Runtime>(
    make: impl FnOnce(&AppHandle<R>) -> Arc<dyn SettingsStorage> + Send + 'static,
) -> TauriPlugin<R> {
    // `setup` takes an `FnOnce`, so the factory waits here until it runs.
    let make = std::sync::Mutex::new(Some(make));
    Builder::new("waypoint-settings")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_settings,
            commands::set_settings,
        ])
        .setup(move |app, _api| {
            let make = make
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
                .ok_or("the settings plugin was set up twice")?;
            let storage = make(app);
            app.manage(SettingsStore::new(app.clone(), storage));
            Ok(())
        })
        .build()
}
