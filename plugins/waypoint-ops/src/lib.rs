// Registers the operations plugin: one queue, one undo journal and one clipboard for every window.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub mod commands;
mod deps;
mod models;
mod ops;
mod worker;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, RunEvent, Runtime, WindowEvent,
};

pub use deps::{ChangeHook, MemorySettings, OpsDeps, SettingsStorage, EXIT_WAIT, SAVE_DELAY};
pub use models::{Clipboard, ClipboardMode, Error, JobJournal, JobProgress, PlanNote, PlanPreview};
pub use ops::{Ops, MAX_CONCURRENCY, MAX_UNDO_DEPTH};

/// The event every change to the queue or the journal is broadcast on, to every window; the
/// payload is an `OpsEvent` carrying the queue's revision (a `JournalChanged` carries the journal's
/// own). Progress ticks are not events: a window subscribes to them on a channel.
pub const EVENT: &str = "waypoint-ops://event";

/// Sent to every window when the shared clipboard changes, with the `Clipboard`.
pub const CLIPBOARD_EVENT: &str = "waypoint-ops://clipboard";

/// Sent to every window when a job records its journal entry, with a `JobJournal`. The job's
/// `JobChanged` events carry `undoable`; this names the entry, and `journal_entry_of` answers the
/// same for a window that missed the event.
pub const JOB_JOURNAL_EVENT: &str = "waypoint-ops://job-journal";

/// Sent once at start-up, with a `RecoveryReport`, when the last run left something interrupted or
/// the journal could not be read. A window that opens later reads it with `take_recovery_report`.
pub const RECOVERED_EVENT: &str = "waypoint-ops://recovered";

/// Registers the plugin with its dependencies ready.
pub fn init<R: Runtime>(deps: OpsDeps) -> TauriPlugin<R> {
    init_with(move |_| deps)
}

/// Registers the plugin with its dependencies made when the plugin is set up, which is when the
/// app exists: the adapters over the Trash and the listings need the app handle, and the journal is
/// opened (and recovered) right after. Register it after the plugins those adapters reach.
pub fn init_with<R: Runtime>(
    make: impl FnOnce(&tauri::AppHandle<R>) -> OpsDeps + Send + 'static,
) -> TauriPlugin<R> {
    // `setup` takes an `FnOnce`, so the factory waits here until it runs.
    let make = std::sync::Mutex::new(Some(make));
    Builder::new("waypoint-ops")
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_snapshot,
            commands::plan,
            commands::preview_batch_rename,
            commands::submit,
            commands::pause,
            commands::resume,
            commands::cancel,
            commands::retry,
            commands::dismiss,
            commands::dismiss_finished,
            commands::reorder,
            commands::resolve,
            commands::resolve_error,
            commands::undo,
            commands::redo,
            commands::journal_summaries,
            commands::journal_entry_of,
            commands::subscribe_progress,
            commands::unsubscribe_progress,
            commands::set_clipboard,
            commands::get_clipboard,
            commands::jobs_targeting,
            commands::get_settings,
            commands::set_settings,
            commands::take_recovery_report,
        ])
        .setup(move |app, _api| {
            let make = make
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
                .ok_or("the operations plugin was set up twice")?;
            let deps = make(app);
            app.manage(Ops::new(app.clone(), deps));
            Ok(())
        })
        .on_event(|app, event| match event {
            RunEvent::WindowEvent {
                label,
                event: WindowEvent::Destroyed,
                ..
            } => on_window_destroyed(app, label),
            RunEvent::Exit => on_exit(app),
            _ => {}
        })
        .build()
}

/// A window that closes stops hearing progress and the journal is written. Its jobs go on: the
/// queue belongs to the app, not to a window.
pub fn on_window_destroyed<R: Runtime>(app: &tauri::AppHandle<R>, label: &str) {
    if let Some(ops) = app.try_state::<Ops<R>>() {
        ops.window_closed(label);
    }
}

/// The app is exiting: running jobs are cancelled so they remove their partial files, and the
/// journal is written.
pub fn on_exit<R: Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(ops) = app.try_state::<Ops<R>>() {
        ops.shutdown(ops.exit_wait());
    }
}
