// Registers the settings plugin: one document for every window, a change event, a status command
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
mod error;
mod folder_views;
mod state;
#[cfg(test)]
mod tests;
mod transfer;

use std::sync::Arc;

use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, Runtime,
};
use waypoint_settings::{FolderViewsStorage, MemoryFolderViews, SettingsStorage};

pub use error::Error;
pub use folder_views::{FolderViewsConfigFile, FolderViewsStore};
pub use state::{ChangeHook, SettingsStore};
pub use transfer::{FilePicker, SettingsConfigFile, Transfers};

/// Sent to every window after every change, with a `SettingsSnapshot` whose revision grows by one
/// each time. A window reads `get_settings` first and applies events with a higher revision.
pub const EVENT: &str = "waypoint-settings://changed";

/// Sent to every window after every change to the remembered folder views, with a
/// `FolderViewsChanged`: the revision it made and the folders it touched. A window reads
/// `get_folder_views` first and applies events whose revision is exactly the next one.
pub const FOLDER_VIEWS_EVENT: &str = "waypoint-settings://folder-views";

/// Registers the plugin over a storage that is ready.
pub fn init<R: Runtime>(storage: Arc<dyn SettingsStorage>) -> TauriPlugin<R> {
    init_with(move |_| storage)
}

/// Registers the plugin with its storage made when the plugin is set up, which is when the app
/// exists: the file it saves in is opened through `tauri-plugin-store`, so register this after it.
pub fn init_with<R: Runtime>(
    make: impl FnOnce(&AppHandle<R>) -> Arc<dyn SettingsStorage> + Send + 'static,
) -> TauriPlugin<R> {
    init_with_folder_views(make, |_| Arc::new(MemoryFolderViews::default()))
}

/// Registers the plugin with the storage of the settings and of the remembered folder views, each
/// made when the plugin is set up. `init_with` keeps the folder views in memory.
pub fn init_with_folder_views<R: Runtime>(
    make: impl FnOnce(&AppHandle<R>) -> Arc<dyn SettingsStorage> + Send + 'static,
    make_views: impl FnOnce(&AppHandle<R>) -> Arc<dyn FolderViewsStorage> + Send + 'static,
) -> TauriPlugin<R> {
    // `setup` takes an `FnOnce`, so the factories wait here until it runs.
    let make = std::sync::Mutex::new(Some(make));
    let make_views = std::sync::Mutex::new(Some(make_views));
    Builder::new("waypoint-settings")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_settings,
            commands::set_settings,
            commands::set_ui_settings,
            commands::export_settings,
            commands::plan_settings_import,
            commands::apply_settings_import,
            commands::get_folder_views,
            commands::remember_folder_view,
            commands::reset_folder_view,
        ])
        .setup(move |app, _api| {
            let make = make
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
                .ok_or("the settings plugin was set up twice")?;
            let storage = make(app);
            app.manage(SettingsStore::new(app.clone(), storage));
            let make_views = make_views
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
                .ok_or("the settings plugin was set up twice")?;
            app.manage(FolderViewsStore::new(app.clone(), make_views(app)));
            let transfers = Transfers::default();
            transfers.register(Arc::new(SettingsConfigFile(app.clone())));
            transfers.register(Arc::new(FolderViewsConfigFile(app.clone())));
            app.manage(transfers);
            Ok(())
        })
        .build()
}
