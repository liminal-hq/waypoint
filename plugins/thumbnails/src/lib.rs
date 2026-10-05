// Registers the thumbnails plugin: a cancellable thumbnail queue over the freedesktop.org cache or the Windows shell, and the `thumb://` scheme
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
mod service;

pub mod builtin;
pub mod cache;
pub mod engine;
pub mod memcache;
pub mod mime;
pub mod models;
pub mod pipeline;
pub mod queue;
pub mod scheme;
pub mod thumbnailer;
pub mod unsupported;
pub mod winmap;

#[cfg(unix)]
pub mod external;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
use linux as platform;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
use windows as platform;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
use unsupported as platform;

use std::sync::Arc;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

use engine::{Engine, Limits};
use memcache::MemCache;

pub use models::*;
pub use platform::Env;
pub use queue::Sink;
pub use service::Thumbnails;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the thumbnail APIs.
pub trait ThumbnailsExt<R: Runtime> {
    fn thumbnails(&self) -> &Thumbnails;
}

impl<R: Runtime, T: Manager<R>> ThumbnailsExt<R> for T {
    fn thumbnails(&self) -> &Thumbnails {
        self.state::<Thumbnails>().inner()
    }
}

/// Initialises the plugin over the real system with the default configuration.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    init_with_config(Config::default())
}

/// Initialises the plugin over the real system.
pub fn init_with_config<R: Runtime>(config: Config) -> TauriPlugin<R> {
    build(config, |app| Env::system(app))
}

/// Initialises the plugin over an injected environment, which tests use to work in a temporary directory.
pub fn init_with_env<R: Runtime>(config: Config, env: Env) -> TauriPlugin<R> {
    build(config, move |_| env)
}

fn build<R: Runtime>(
    config: Config,
    env: impl FnOnce(&tauri::AppHandle<R>) -> Env + Send + 'static,
) -> TauriPlugin<R> {
    Builder::new("thumbnails")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::request,
            commands::cancel,
            commands::prioritise,
        ])
        .register_uri_scheme_protocol(scheme::SCHEME, |ctx, request| {
            match ctx.app_handle().try_state::<Thumbnails>() {
                Some(thumbnails) => {
                    scheme::respond(thumbnails.inner(), request.method(), request.uri().path())
                }
                None => tauri::http::Response::builder()
                    .status(tauri::http::StatusCode::SERVICE_UNAVAILABLE)
                    .body(Vec::new())
                    .expect("a status-only response is valid"),
            }
        })
        .setup(move |app, _api| {
            app.manage(start(&config, env(app)));
            Ok(())
        })
        .build()
}

/// Builds the cache, the queue and its workers over `env`.
fn start(config: &Config, env: Env) -> Thumbnails {
    let mem = Arc::new(MemCache::new(config.memory_cache_bytes));
    let limits = Arc::new(Limits::new(config.max_file_bytes, config.external_timeout));
    let platform = platform::Platform::new(env, config, Arc::clone(&mem), Arc::clone(&limits));
    let engine = Engine::new(config.worker_count(), platform.processor());
    let bytes_mem = Arc::new(MemCache::new(config.bytes_cache_bytes));
    Thumbnails::new(
        engine,
        platform.store(),
        mem,
        bytes_mem,
        limits,
        platform.status(),
    )
}
