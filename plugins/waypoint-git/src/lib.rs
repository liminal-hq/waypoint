// Registers the Git plugin: the status of the working trees folders are in, for every window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod commands;
mod error;
mod overlay;
mod state;
mod summary;
#[cfg(test)]
mod tests;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, RunEvent, Runtime, WindowEvent,
};

pub use error::Error;
pub use overlay::GitOverlay;
pub use state::Git;
pub use summary::wire as wire_summary;

/// Sent to the window that watches a repository when its state changes, with a `GitChanged`. A
/// window reads the reply of `git_watch` first and applies events with a higher revision.
pub const CHANGED_EVENT: &str = "waypoint-git://changed";

/// Ends the watches a window made when it is destroyed.
fn on_run_event<R: Runtime>(app: &tauri::AppHandle<R>, event: &RunEvent) {
    if let RunEvent::WindowEvent {
        label,
        event: WindowEvent::Destroyed,
        ..
    } = event
    {
        if let Some(git) = app.try_state::<Git>() {
            let ended = git.forget_window(label);
            if ended > 0 {
                log::debug!("ended {ended} Git watches with window={label}");
            }
        }
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("waypoint-git")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::git_watch,
            commands::git_unwatch,
            commands::git_badges,
        ])
        .setup(|app, _api| {
            app.manage(Git::default());
            Ok(())
        })
        .on_event(on_run_event)
        .build()
}
