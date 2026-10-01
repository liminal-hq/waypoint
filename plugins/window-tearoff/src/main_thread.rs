// Runs a job on the thread that owns the windows and awaits its result
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use tauri::{AppHandle, Runtime};
use tokio::sync::oneshot;

/// Runs `job` on the main thread and awaits its result; `None` if the event loop is gone. GTK and Win32 window calls require the thread that owns the window.
pub async fn run<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    job: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(job());
    })
    .ok()?;
    rx.await.ok()
}

/// Runs `job` on the main thread and blocks the calling (non-main) thread for its result; `None` if the event loop is gone.
pub fn run_blocking<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    job: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = tx.send(job());
    })
    .ok()?;
    rx.recv().ok()
}
