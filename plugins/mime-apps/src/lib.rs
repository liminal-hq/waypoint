// Registers the mime-apps plugin: the type of a file, the applications that open it, Open With, the default handler and application icons, on Linux and Windows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub mod assoc;
pub mod backend;
mod commands;
pub mod directory;
mod error;
pub mod mimeapps;
pub mod models;
pub mod scheme;
mod service;
pub mod status;
pub mod target;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
use linux as platform;

#[cfg(target_os = "windows")]
pub mod windows;
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

pub use backend::{Backend, ParentWindow};
pub use directory::{AppDirectory, AppEntry, DirectoryBackend};
pub use error::{MimeAppsError, Result};
pub use models::*;
pub use service::MimeApps;
pub use target::Target;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the mime-apps APIs.
pub trait MimeAppsExt<R: Runtime> {
    fn mime_apps(&self) -> &MimeApps<R>;
}

impl<R: Runtime, T: Manager<R>> MimeAppsExt<R> for T {
    fn mime_apps(&self) -> &MimeApps<R> {
        self.state::<MimeApps<R>>().inner()
    }
}

/// Initialises the plugin over the real system: gio on Linux, the shell's association handlers on Windows.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    build(|app| Arc::new(platform::Platform::for_app(app)))
}

/// Initialises the plugin over a backend of the caller's, which tests use to avoid starting anything.
pub fn init_with<R: Runtime>(backend: Arc<dyn Backend>) -> TauriPlugin<R> {
    build(move |_| backend)
}

fn build<R: Runtime>(
    backend: impl FnOnce(&tauri::AppHandle<R>) -> Arc<dyn Backend> + Send + 'static,
) -> TauriPlugin<R> {
    Builder::new("mime-apps")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::type_info,
            commands::handlers,
            commands::open_with,
            commands::open_default,
            commands::choose,
            commands::set_default,
            commands::open_default_apps_settings,
        ])
        .register_uri_scheme_protocol(scheme::SCHEME, |ctx, request| {
            match ctx.app_handle().try_state::<MimeApps<R>>() {
                Some(mime_apps) => {
                    let (backend, icons) = mime_apps.scheme_parts();
                    scheme::respond(
                        backend.as_ref(),
                        &icons,
                        request.method(),
                        request.uri().path(),
                        request.uri().query(),
                    )
                }
                None => tauri::http::Response::builder()
                    .status(tauri::http::StatusCode::SERVICE_UNAVAILABLE)
                    .body(Vec::new())
                    .expect("a status-only response is valid"),
            }
        })
        .setup(move |app, _api| {
            let backend = backend(app);
            app.manage(MimeApps::new(app.clone(), backend));
            Ok(())
        })
        .build()
}
