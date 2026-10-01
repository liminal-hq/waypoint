// Registers the window tear-off plugin: a click-through ghost that follows the cursor, and drop hit-testing across windows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
mod error;
pub mod follow;
mod ghost;
mod main_thread;
pub mod models;
pub mod regions;
mod session;
pub mod status;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as platform;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
use unsupported as platform;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, RunEvent, Runtime, WindowEvent,
};

pub use error::Error;
pub use models::Options;

use session::Tearoff;

#[cfg(test)]
mod tests;

/// Registers the plugin and pre-creates the ghost window. Tell any window-state plugin to ignore `Options::window_state_denylist`.
pub fn init<R: Runtime>(options: Options) -> TauriPlugin<R> {
    Builder::new("window-tearoff")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::begin,
            commands::update,
            commands::end,
            commands::set_drop_regions,
            commands::get_cursor,
            commands::get_payload,
            commands::hit_test,
        ])
        .setup(move |app, _api| {
            app.manage(Tearoff::new(options));
            Ok(())
        })
        .on_event(|app, event| match event {
            // The ghost is created once the event loop is running, not in `setup`: Tauri holds its plugin store locked while plugins set up, and creating a window needs that store, so a window built in `setup` deadlocks.
            RunEvent::Ready => {
                if let Some(state) = app.try_state::<Tearoff>() {
                    state.create_ghost(app);
                }
            }
            RunEvent::WindowEvent {
                label,
                event: WindowEvent::Destroyed,
                ..
            } => {
                if let Some(state) = app.try_state::<Tearoff>() {
                    state.window_destroyed(app, label);
                }
            }
            _ => {}
        })
        .build()
}
