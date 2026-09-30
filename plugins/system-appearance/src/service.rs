// Keeps the last-known preferences and emits an event when they change
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    sync::{Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::sync::mpsc;

use crate::{models::Snapshot, platform};

/// Event emitted to all windows with the new `TitlebarPreferences` when they change.
pub const CHANGED_EVENT: &str = "system-appearance://titlebar-preferences-changed";

/// How long to let a burst of change notifications settle before re-reading.
const DEBOUNCE: Duration = Duration::from_millis(150);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Plugin state: the last reading and the platform's change watcher.
#[derive(Default)]
pub struct Service {
    /// Serialises reads so a slow older reading cannot overwrite a newer one.
    gate: tokio::sync::Mutex<()>,
    last: Mutex<Option<Snapshot>>,
    watcher: Mutex<Option<platform::Watcher>>,
}

impl Service {
    /// Reads the platform now, remembers the result and emits the change event if the
    /// preferences differ from the previous reading. The first reading only sets the baseline.
    pub async fn refresh<R: Runtime>(&self, app: &AppHandle<R>) -> Snapshot {
        let _turn = self.gate.lock().await;
        let next = platform::read().await;
        let previous = lock(&self.last).replace(next.clone());
        if previous.is_some_and(|previous| previous.preferences != next.preferences) {
            if let Err(error) = app.emit(CHANGED_EVENT, &next.preferences) {
                log::warn!("system-appearance: failed to emit change event: {error}");
            }
        }
        next
    }

    /// Starts watching the platform and re-reads whenever it reports a change.
    pub fn start<R: Runtime>(&self, app: &AppHandle<R>) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        *lock(&self.watcher) = Some(platform::watch(tx));

        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            app.state::<Service>().refresh(&app).await;
            while rx.recv().await.is_some() {
                tokio::time::sleep(DEBOUNCE).await;
                while rx.try_recv().is_ok() {}
                app.state::<Service>().refresh(&app).await;
            }
        });
    }

    /// Stops the platform watcher, releasing any child processes.
    pub fn stop(&self) {
        lock(&self.watcher).take();
    }
}
