// Registers the window-effects plugin: report which window effects work and apply them per window, on Windows and Linux
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub mod backend;
mod commands;
mod error;
#[cfg(target_os = "linux")]
mod ffi;
pub mod models;
pub mod request;
mod service;
pub mod status;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_os = "linux")]
mod x11;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as platform;

// Compiled everywhere for its tests; used only where there is no other backend.
#[cfg(any(test, not(any(target_os = "linux", target_os = "windows"))))]
mod unsupported;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
use unsupported as platform;

use std::sync::Arc;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

#[cfg(target_os = "linux")]
pub use linux::probe_environment;
#[cfg(target_os = "windows")]
pub use windows::{build_number, probe_environment};

pub use backend::{Backend, BoxFuture};
pub use error::{Result, WindowEffectsError};
pub use models::*;
pub use service::WindowEffects;
pub use status::{
    blur_path, status_for, BlurPath, Desktop, Environment, SessionType, EXT_BACKGROUND_EFFECT,
    KDE_BLUR,
};

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the window-effects APIs.
pub trait WindowEffectsExt<R: Runtime> {
    fn window_effects(&self) -> &WindowEffects<R>;
}

impl<R: Runtime, T: Manager<R>> WindowEffectsExt<R> for T {
    fn window_effects(&self) -> &WindowEffects<R> {
        self.state::<WindowEffects<R>>().inner()
    }
}

/// Initialises the plugin over the real system: Wayland, X11 and GTK on Linux, DWM on Windows.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    init_with(Arc::new(platform::Platform::new()))
}

/// Initialises the plugin over a backend of the caller's: a fake in a test.
pub fn init_with<R: Runtime>(backend: Arc<dyn Backend<R>>) -> TauriPlugin<R> {
    Builder::new("window-effects")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::apply,
            commands::clear,
            commands::set_shadow_inset,
        ])
        .setup(move |app, _api| {
            app.manage(WindowEffects::new(app.clone(), backend));
            Ok(())
        })
        .build()
}
