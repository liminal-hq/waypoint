// Registers the file system plugin: listing commands, places and favourites
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
mod error;
mod registry;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, RunEvent, Runtime, WindowEvent,
};

pub use commands::{OpenOptions, Vfs, LISTING_EVENT};
pub use error::Error;

/// Closes every listing a window opened, and stops their scans and watchers, when it is destroyed.
fn on_window_event<R: Runtime>(app: &tauri::AppHandle<R>, label: &str, event: &WindowEvent) {
    if matches!(event, WindowEvent::Destroyed) {
        if let Some(vfs) = app.try_state::<Vfs>() {
            let closed = vfs.registry.close_window(label);
            if closed > 0 {
                log::debug!("closed {closed} listings with window={label}");
            }
        }
    }
}

fn on_run_event<R: Runtime>(app: &tauri::AppHandle<R>, event: &RunEvent) {
    if let RunEvent::WindowEvent { label, event, .. } = event {
        on_window_event(app, label, event);
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("waypoint-vfs")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::open_listing,
            commands::get_range,
            commands::set_sort,
            commands::set_filter,
            commands::close_listing,
            commands::get_home,
            commands::list_places,
            commands::add_favourite,
            commands::remove_favourite,
            commands::rename_favourite,
            commands::move_favourite,
        ])
        .setup(|app, _api| {
            app.manage(Vfs::default());
            Ok(())
        })
        .on_event(on_run_event)
        .build()
}

#[cfg(all(test, unix))]
mod tests;
