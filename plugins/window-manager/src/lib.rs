// Registers the window manager plugin commands for Tauri
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
mod error;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod main_thread;
pub mod models;
pub mod session;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

#[cfg(any(target_os = "windows", test))]
mod system_menu;
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
    Runtime,
};

pub use error::Error;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("window-manager")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_capabilities,
            commands::show_system_window_menu,
            commands::get_always_on_top,
        ])
        .on_window_ready(|window| platform::watch_always_on_top(&window))
        .build()
}
