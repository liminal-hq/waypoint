// Registers the secrets plugin: store, fetch and delete passwords, passphrases and tokens in the system keyring
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub mod backend;
mod commands;
mod error;
pub mod memory;
pub mod models;
mod service;

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

pub use backend::{Backend, BoxFuture};
pub use error::{Result, SecretsError, MAX_NAME};
pub use memory::MemoryBackend;
pub use models::*;
pub use service::Secrets;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the secrets APIs.
pub trait SecretsExt<R: Runtime> {
    fn secrets(&self) -> &Secrets;
}

impl<R: Runtime, T: Manager<R>> SecretsExt<R> for T {
    fn secrets(&self) -> &Secrets {
        self.state::<Secrets>().inner()
    }
}

/// Initialises the plugin over the real keyring. Secrets are filed under the app's identifier, so two apps never see each other's.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    build(None)
}

/// The system keyring on its own, for a caller that wants a [`Secrets`] without a Tauri app (the live test, a command-line tool). `namespace` is the app identifier items are filed under.
pub fn system_backend(namespace: impl Into<String>) -> Arc<dyn Backend> {
    Arc::new(platform::Platform::new(namespace.into()))
}

/// Initialises the plugin over a backend of the caller's: a fake in a test.
pub fn init_with<R: Runtime>(backend: Arc<dyn Backend>) -> TauriPlugin<R> {
    build(Some(backend))
}

fn build<R: Runtime>(backend: Option<Arc<dyn Backend>>) -> TauriPlugin<R> {
    Builder::new("secrets")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::store,
            commands::fetch,
            commands::exists,
            commands::delete,
            commands::delete_account,
        ])
        .setup(move |app, _api| {
            let backend = backend.unwrap_or_else(|| {
                Arc::new(platform::Platform::new(app.config().identifier.clone()))
            });
            app.manage(Secrets::new(backend));
            Ok(())
        })
        .build()
}
