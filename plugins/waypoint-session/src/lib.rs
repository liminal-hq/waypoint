// Registers the session plugin: one session per window, keyed by the window's label.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
mod error;
mod sessions;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, RunEvent, Runtime, WindowEvent,
};

pub use error::Error;
pub use sessions::Sessions;

/// The event every window's session changes arrive on; the payload is a `SessionEvent`.
pub const EVENT: &str = "waypoint-session://event";

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("waypoint-session")
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::open_tab,
            commands::close_tab,
            commands::activate_tab,
            commands::move_tab,
            commands::navigate,
            commands::back,
            commands::forward,
            commands::get_status,
        ])
        .setup(|app, _api| {
            app.manage(Sessions::default());
            Ok(())
        })
        .on_event(|app, event| {
            // A window that closes takes its tabs with it, so hooks release what they held.
            if let RunEvent::WindowEvent {
                label,
                event: WindowEvent::Destroyed,
                ..
            } = event
            {
                app.state::<Sessions>().end(label);
            }
        })
        .build()
}
