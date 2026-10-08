// Registers the elevate plugin: starts a helper program with administrator rights through the system's prompt, from Rust code only
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
pub mod launch;
pub mod models;
pub mod polkit;
pub mod probe;

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

use std::path::PathBuf;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use launch::{ElevatedStream, LaunchError};
pub use models::*;

/// Why every platform without a launcher reports unavailable.
pub const REASON_NOT_IMPLEMENTED: &str = "Not implemented on this platform yet";

/// What the app supplies: which helper to start and how to recognise that it is running. The plugin has no default for any of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The absolute path of the helper program, installed root-owned in a root-owned folder. On Linux the polkit policy names this same path.
    pub helper: PathBuf,
    /// The absolute path of the installed polkit policy file that authorises it.
    pub policy: PathBuf,
    /// The one line the helper writes to its error stream, as its first act, when it is running. This is a wire contract with the helper: [`launch`] waits for exactly this line, because the system's prompt can keep the helper from starting for as long as the person likes.
    pub ready_line: String,
}

impl Config {
    pub fn new(
        helper: impl Into<PathBuf>,
        policy: impl Into<PathBuf>,
        ready_line: impl Into<String>,
    ) -> Self {
        Config {
            helper: helper.into(),
            policy: policy.into(),
            ready_line: ready_line.into(),
        }
    }
}

/// Starts the helper and reports whether it can be started.
pub struct Elevator {
    platform: platform::Platform,
}

impl Elevator {
    pub fn new(config: Config) -> Self {
        Elevator {
            platform: platform::Platform::new(config),
        }
    }

    /// What works on this system, and for the first failing check, why not. Read again on every call.
    pub fn status(&self) -> PluginStatus {
        self.platform.status()
    }

    /// Starts the helper through the system's prompt and blocks until it is running, however long the person takes at the prompt. `cancelled` is polled about every 50 ms; when it returns true the start is abandoned with [`LaunchError::Cancelled`]. Run it on a thread of its own. The status is checked first, and a failing check is [`LaunchError::Unavailable`].
    pub fn launch(&self, cancelled: &dyn Fn() -> bool) -> Result<ElevatedStream, LaunchError> {
        self.platform.launch(cancelled)
    }
}

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to reach the elevator from Rust.
pub trait ElevateExt<R: Runtime> {
    fn elevate(&self) -> &Elevator;
}

impl<R: Runtime, T: Manager<R>> ElevateExt<R> for T {
    fn elevate(&self) -> &Elevator {
        self.state::<Elevator>().inner()
    }
}

/// Initialises the plugin for a helper. The only command it gives the page is `get_status`: starting a helper is Rust-only, so script in a webview can never cause a prompt or an elevated process.
pub fn init_with_config<R: Runtime>(config: Config) -> TauriPlugin<R> {
    Builder::new("elevate")
        .invoke_handler(tauri::generate_handler![commands::get_status])
        .setup(move |app, _api| {
            app.manage(Elevator::new(config));
            Ok(())
        })
        .build()
}
