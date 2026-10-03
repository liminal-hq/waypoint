// Registers the system appearance plugin commands and change watcher for Tauri
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub mod appearance;
mod commands;
mod error;
pub mod models;
pub mod palette;
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

pub use appearance::service::APPEARANCE_CHANGED_EVENT;
pub use error::Error;
pub use palette::service::PALETTE_CHANGED_EVENT;
pub use service::CHANGED_EVENT;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("system-appearance")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_titlebar_preferences,
            commands::get_appearance,
            commands::get_palette,
        ])
        .setup(|app, _api| {
            app.manage(service::Service::default());
            app.state::<service::Service>().start(app);
            app.manage(appearance::service::AppearanceService::default());
            app.state::<appearance::service::AppearanceService>()
                .start(app);
            app.manage(palette::service::PaletteService::default());
            app.state::<palette::service::PaletteService>().start(app);
            Ok(())
        })
        .on_event(|app, event| {
            if let RunEvent::Exit = event {
                app.state::<service::Service>().stop();
                app.state::<appearance::service::AppearanceService>().stop();
                app.state::<palette::service::PaletteService>().stop();
            }
        })
        .build()
}
