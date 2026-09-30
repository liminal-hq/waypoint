// Registers the system appearance plugin commands and change watcher for Tauri
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
mod error;
pub mod models;
pub mod parse;
mod service;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as platform;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
use unsupported as platform;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, RunEvent, Runtime,
};

pub use error::Error;
pub use service::CHANGED_EVENT;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("system-appearance")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_titlebar_preferences,
        ])
        .setup(|app, _api| {
            app.manage(service::Service::default());
            app.state::<service::Service>().start(app);
            Ok(())
        })
        .on_event(|app, event| {
            if let RunEvent::Exit = event {
                app.state::<service::Service>().stop();
            }
        })
        .build()
}
