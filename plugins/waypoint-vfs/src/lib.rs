// Registers the file system plugin: listing commands, places and favourites
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
mod connections;
mod error;
mod registry;
mod wpfile;

use std::sync::Arc;

use tauri::{
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, RunEvent, Runtime, WindowEvent,
};
use waypoint_connections::{
    ConnectionManager, ConnectionStorage, ConnectionsHub, Credentials, MemoryConnections,
    NoKeyring, SWEEP_EVERY,
};
use waypoint_vfs::{Provider, ProviderRegistry};

pub use commands::{OpenOptions, Suggestions, Vfs, LISTING_EVENT};
pub use connections::{
    announce_protocols, CONNECTIONS_EVENT, CONNECTION_STATE_EVENT, PROTOCOLS_EVENT,
};
pub use error::Error;
pub use wpfile::SCHEME as PREVIEW_SCHEME;

/// Closes every listing a window opened, and stops their scans and watchers, when it is destroyed.
fn on_window_event<R: Runtime>(app: &tauri::AppHandle<R>, label: &str, event: &WindowEvent) {
    if matches!(event, WindowEvent::Destroyed) {
        if let Some(vfs) = app.try_state::<Vfs>() {
            let stopped = vfs.size_jobs.cancel_window(label);
            if stopped > 0 {
                log::debug!("cancelled {stopped} folder sizes with window={label}");
            }
            let closed = vfs.registry.close_window(label);
            if closed > 0 {
                log::debug!("closed {closed} listings with window={label}");
            }
        }
    }
}

fn on_run_event<R: Runtime>(app: &tauri::AppHandle<R>, event: &RunEvent) {
    if let RunEvent::WindowEvent { label, event, .. } = event {
        on_window_event(app, label, event);
    }
}

/// Makes the storage of the saved connections once the app exists.
pub type StorageFactory<R> = Box<dyn FnOnce(&AppHandle<R>) -> Arc<dyn ConnectionStorage> + Send>;

/// What the app gives the plugin (A85, A81): the server, archive and Git providers it registers,
/// the credential source those providers were built with (so a login the person answers reaches
/// them), where the saved connections are kept, and the SSH hosts to suggest. The plugin calls no
/// other plugin: the keyring is behind `credentials`, and the storage is the app's.
pub struct Options<R: Runtime> {
    pub providers: Vec<Arc<dyn Provider>>,
    pub credentials: Option<Arc<Credentials>>,
    pub storage: Option<StorageFactory<R>>,
    pub suggestions: Option<Suggestions>,
}

impl<R: Runtime> Default for Options<R> {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            credentials: None,
            storage: None,
            suggestions: None,
        }
    }
}

/// The plugin with local folders and the Trash only: no remote provider, and saved connections
/// kept in memory with no keyring.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    init_with(Options::default())
}

/// The plugin with the app's providers and connections.
pub fn init_with<R: Runtime>(options: Options<R>) -> TauriPlugin<R> {
    let Options {
        providers,
        credentials,
        storage,
        suggestions,
    } = options;
    let mut storage = Some(storage);
    Builder::new("waypoint-vfs")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_trash_info,
            commands::open_listing,
            commands::get_range,
            commands::set_sort,
            commands::set_filter,
            commands::close_listing,
            commands::refresh_listing,
            commands::get_home,
            commands::parse_location,
            commands::parse_location_text,
            commands::describe_location,
            commands::entry_location,
            commands::summarise_selection,
            commands::get_free_space,
            commands::check_folder,
            commands::open_entry,
            commands::entry_details,
            commands::folder_size,
            commands::cancel_folder_size,
            commands::scan_dir_sizes,
            commands::cancel_dir_scan,
            commands::get_cached_dir_scan,
            commands::read_text_head,
            commands::list_places,
            commands::add_favourite,
            commands::remove_favourite,
            commands::rename_favourite,
            commands::move_favourite,
            connections::list_connections,
            connections::connection_support,
            connections::suggested_servers,
            connections::parse_address_text,
            connections::add_connection,
            connections::update_connection,
            connections::duplicate_connection,
            connections::remove_connection,
            connections::move_connection,
            connections::forget_recent_server,
            connections::forget_login,
            connections::connect,
            connections::cancel_connect,
            connections::test_connection,
            connections::disconnect,
            connections::connection_state,
        ])
        .register_asynchronous_uri_scheme_protocol(wpfile::SCHEME, |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let window = ctx.webview_label().to_owned();
            tauri::async_runtime::spawn_blocking(move || {
                let served = match app.try_state::<Vfs>() {
                    Some(vfs) => wpfile::respond(
                        &vfs.registry,
                        &window,
                        request.method().as_str(),
                        request.uri().path(),
                        request
                            .headers()
                            .get(tauri::http::header::RANGE)
                            .and_then(|value| value.to_str().ok()),
                    ),
                    None => waypoint_vfs::status_response(503, "not ready"),
                };
                responder.respond(wpfile::into_response(served));
            });
        })
        .setup(move |app, _api| {
            let registry = Arc::new(ProviderRegistry::new());
            for provider in &providers {
                registry.register(provider.clone());
            }
            let credentials = credentials
                .clone()
                .unwrap_or_else(|| Arc::new(Credentials::new(Arc::new(NoKeyring))));
            let storage: Arc<dyn ConnectionStorage> = match storage.take().flatten() {
                Some(make) => make(app),
                None => Arc::new(MemoryConnections::default()),
            };
            let manager = Arc::new(ConnectionManager::new(registry.clone(), credentials));
            manager.spawn_sweeper(SWEEP_EVERY);
            let hub = Arc::new(ConnectionsHub::new(storage, manager));
            connections::wire_events(app, &hub);
            app.manage(Vfs::new(registry, Some(hub), suggestions.clone()));
            Ok(())
        })
        .on_event(on_run_event)
        .build()
}

#[cfg(all(test, unix))]
mod tests;
