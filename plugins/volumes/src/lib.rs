// Registers the volumes plugin: list drives, mounts and network shares with their free space, and mount, unmount, eject and unlock them, on Linux and Windows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub mod backend;
mod commands;
mod debounce;
mod error;
mod holders;
pub mod models;
pub mod mountinfo;
mod service;
pub mod space;
pub mod udisks;
mod winmap;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as platform;

// Compiled everywhere for its tests; used only where there is no other backend.
#[cfg(any(test, not(target_os = "linux")))]
mod unsupported;
#[cfg(not(target_os = "linux"))]
use unsupported as platform;

use std::sync::Arc;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use backend::{Backend, BoxFuture, Notify};
pub use debounce::Debounce;
pub use error::{Result, VolumesError};
pub use models::*;
pub use service::{Options, Volumes};
pub use space::{Space, SpaceFn};

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the volumes APIs.
pub trait VolumesExt<R: Runtime> {
    fn volumes(&self) -> &Volumes<R>;
}

impl<R: Runtime, T: Manager<R>> VolumesExt<R> for T {
    fn volumes(&self) -> &Volumes<R> {
        self.state::<Volumes<R>>().inner()
    }
}

/// Initialises the plugin over the real system: UDisks2 and the mount table on Linux, the drive APIs on Windows.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    init_with(Arc::new(platform::Platform::new()), Options::default())
}

/// Initialises the plugin over a backend and options of the caller's: a fake in a test, or other timeouts.
pub fn init_with<R: Runtime>(backend: Arc<dyn Backend>, options: Options) -> TauriPlugin<R> {
    Builder::new("volumes")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::list,
            commands::refresh_space,
            commands::mount,
            commands::unmount,
            commands::eject,
            commands::unlock,
        ])
        .setup(move |app, _api| {
            let volumes = Volumes::new(app.clone(), backend, options);
            let core = volumes.core();
            app.manage(volumes);
            // The first list, then every change: the windows hear of both as `volumes://changed`.
            tauri::async_runtime::spawn(core.run());
            Ok(())
        })
        .build()
}
