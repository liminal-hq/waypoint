// Registers the os-prefs plugin commands and, on desktop, the time-format change watcher
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(desktop)]
use tauri::RunEvent;
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

mod commands;
mod error;
pub mod models;
pub mod parse;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;
#[cfg(desktop)]
mod service;

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple;
#[cfg(all(desktop, target_os = "linux"))]
mod linux;
#[cfg(all(desktop, target_os = "linux"))]
use linux as platform;

#[cfg(all(desktop, target_os = "macos"))]
mod macos;
#[cfg(all(desktop, target_os = "macos"))]
use macos as platform;

#[cfg(all(desktop, target_os = "windows"))]
mod windows;
#[cfg(all(desktop, target_os = "windows"))]
use windows as platform;

#[cfg(all(
    desktop,
    not(any(target_os = "linux", target_os = "macos", target_os = "windows"))
))]
mod unsupported;
#[cfg(all(
    desktop,
    not(any(target_os = "linux", target_os = "macos", target_os = "windows"))
))]
use unsupported as platform;

#[cfg(desktop)]
use desktop::OsPrefs;
#[cfg(mobile)]
use mobile::OsPrefs;

pub use error::{Error, Result};
pub use models::*;
#[cfg(desktop)]
pub use service::CHANGED_EVENT;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the os-prefs APIs.
pub trait OsPrefsExt<R: Runtime> {
    fn os_prefs(&self) -> &OsPrefs<R>;
}

impl<R: Runtime, T: Manager<R>> OsPrefsExt<R> for T {
    fn os_prefs(&self) -> &OsPrefs<R> {
        self.state::<OsPrefs<R>>().inner()
    }
}

/// Initialises the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("os-prefs")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_time_format,
            commands::get_animator_duration_scale,
            commands::open_notification_settings
        ])
        .setup(|app, api| {
            #[cfg(mobile)]
            let os_prefs = mobile::init(app, api)?;
            #[cfg(desktop)]
            let os_prefs = desktop::init(app, api)?;
            app.manage(os_prefs);
            Ok(())
        })
        .on_event(|_app, event| {
            #[cfg(desktop)]
            if let RunEvent::Exit = event {
                _app.state::<service::Service>().stop();
            }
            #[cfg(mobile)]
            let _ = event;
        })
        .build()
}
