// Registers the file system plugin: listing commands, places and favourites
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
mod error;
mod registry;
mod wpfile;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, RunEvent, Runtime, WindowEvent,
};

pub use commands::{OpenOptions, Vfs, LISTING_EVENT};
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

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("waypoint-vfs")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_trash_info,
            commands::open_listing,
            commands::get_range,
            commands::set_sort,
            commands::set_filter,
            commands::close_listing,
            commands::get_home,
            commands::parse_location,
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
        .setup(|app, _api| {
            app.manage(Vfs::default());
            Ok(())
        })
        .on_event(on_run_event)
        .build()
}

#[cfg(all(test, unix))]
mod tests;
