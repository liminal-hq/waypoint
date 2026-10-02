// Registers the trash plugin: move to the trash, list it, restore, delete, empty and expire, on Linux and Windows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
mod error;
pub mod models;
mod service;
mod winmap;

#[cfg(target_os = "linux")]
pub mod freedesktop;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;

#[cfg(target_os = "windows")]
mod main_thread;
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
    Manager, Runtime,
};

pub use error::{Result, TrashError};
pub use models::*;
pub use service::Trash;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the trash APIs.
pub trait TrashExt<R: Runtime> {
    fn trash(&self) -> &Trash<R>;
}

impl<R: Runtime, T: Manager<R>> TrashExt<R> for T {
    fn trash(&self) -> &Trash<R> {
        self.state::<Trash<R>>().inner()
    }
}

/// Initialises the plugin over the real system: the real trash directories and mount table on Linux, the Recycle Bin on Windows.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    build(platform::Platform::new)
}

/// Initialises the plugin over an injected freedesktop environment, which tests use to work in a temporary directory. The Flatpak portal is never used.
#[cfg(target_os = "linux")]
pub fn init_with_env<R: Runtime>(env: freedesktop::TrashEnv) -> TauriPlugin<R> {
    build(move || platform::Platform::with_env(env))
}

fn build<R: Runtime>(
    platform: impl FnOnce() -> platform::Platform + Send + 'static,
) -> TauriPlugin<R> {
    Builder::new("trash")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::trash,
            commands::list,
            commands::restore,
            commands::delete,
            commands::empty,
        ])
        .setup(move |app, _api| {
            app.manage(Trash::new(app.clone(), platform()));
            Ok(())
        })
        .build()
}
