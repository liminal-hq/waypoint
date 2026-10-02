// Registers the session plugin: one store for every window, commands that act on the caller's.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
mod deps;
mod error;
mod sessions;
#[cfg(test)]
mod tests;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, RunEvent, Runtime, WindowEvent,
};

pub use deps::{
    ChangeHook, LastWindowHook, MemoryStorage, SessionDeps, WindowError, WindowFactory,
    CHANGE_DELAY,
};
pub use error::Error;
pub use sessions::{Sessions, MAX_WINDOWS, WARN_WINDOWS};

/// The event every window's session changes arrive on; the payload is a `SessionEvent`.
pub const EVENT: &str = "waypoint-session://event";

/// Sent to a window that tabs were handed to by a command from another window, with a `Handoff`
/// payload, so it can say so. A window made by the hand-off is not sent one: it is not listening yet.
pub const HANDOFF_EVENT: &str = "waypoint-session://handoff";

pub fn init<R: Runtime>(deps: SessionDeps<R>) -> TauriPlugin<R> {
    // `setup` takes an `FnOnce`, so the deps wait here until it runs.
    let deps = std::sync::Mutex::new(Some(deps));
    Builder::new("waypoint-session")
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::open_tab,
            commands::close_tab,
            commands::activate_tab,
            commands::move_tab,
            commands::navigate,
            commands::back,
            commands::forward,
            commands::pin_tab,
            commands::set_tab_colour,
            commands::set_tab_hints,
            commands::reopen_tab,
            commands::create_group,
            commands::add_to_group,
            commands::remove_from_group,
            commands::rename_group,
            commands::set_group_colour,
            commands::set_group_collapsed,
            commands::collapse_other_groups,
            commands::sort_group,
            commands::duplicate_group,
            commands::move_group,
            commands::ungroup,
            commands::close_group,
            commands::save_group_as_workspace,
            commands::rename_workspace,
            commands::delete_workspace,
            commands::set_active_workspace,
            commands::set_workspace_locations,
            commands::add_to_shelf,
            commands::remove_from_shelf,
            commands::clear_shelf,
            commands::move_shelf_item,
            commands::set_shelf_undocked,
            commands::set_shelf_on_top,
            commands::join_pair,
            commands::separate_pair,
            commands::set_pair_layout,
            commands::set_pair_sizes,
            commands::swap_panes,
            commands::toggle_split,
            commands::open_window,
            commands::close_window,
            commands::set_geometry,
            commands::set_view,
            commands::move_tabs,
            commands::list_windows,
            commands::get_status,
        ])
        .setup(move |app, _api| {
            let deps = deps
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
                .ok_or("the session plugin was set up twice")?;
            app.manage(Sessions::new(deps));
            Ok(())
        })
        .on_event(|app, event| {
            if let RunEvent::WindowEvent { label, event, .. } = event {
                on_window_event(app, label, event);
            }
        })
        .build()
}

/// A window that closes takes its tabs with it, so hooks release what they held. The cleanup
/// runs off the main thread: a window factory holds the store's lock while it waits for the main
/// thread to build a window, so the main thread must never wait for that lock.
fn on_window_event<R: Runtime>(app: &tauri::AppHandle<R>, label: &str, event: &WindowEvent) {
    if matches!(event, WindowEvent::Destroyed) {
        let app = app.clone();
        let label = label.to_string();
        tauri::async_runtime::spawn_blocking(move || {
            app.state::<Sessions<R>>().window_destroyed(&app, &label);
        });
    }
}
